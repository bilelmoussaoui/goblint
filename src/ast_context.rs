use anyhow::Result;
use globset::GlobSet;
use gobject_ast::{FunctionInfo, Parser, Project};
use indicatif::ProgressBar;
use std::cell::RefCell;
use std::path::Path;
use walkdir::WalkDir;

/// AST-based project context that replaces the old tree-sitter based ProjectContext
pub struct AstContext {
    pub project: Project,
    /// Tree-sitter parser for rules that need to parse C code
    ts_parser: RefCell<tree_sitter::Parser>,
}

impl AstContext {
    /// Build with ignore patterns
    pub fn build_with_ignore(
        directory: &Path,
        ignore_matcher: &GlobSet,
        spinner: Option<&ProgressBar>,
    ) -> Result<Self> {
        let mut parser = Parser::new()?;
        let mut project = Project::new();

        // Collect all files first to get count
        let files: Vec<_> = WalkDir::new(directory)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "h" || ext == "c")
            })
            .filter(|e| {
                let path = e.path();
                let relative_path = path.strip_prefix(directory).unwrap_or(path);
                !ignore_matcher.is_match(relative_path)
            })
            .collect();

        let total_files = files.len();

        // Parse each file
        for (i, entry) in files.iter().enumerate() {
            let path = entry.path();

            if let Some(sp) = spinner {
                sp.set_message(format!("Parsing files... {}/{}", i + 1, total_files));
            }

            // Parse this file
            if let Ok(file_project) = parser.parse_file(path) {
                // Merge into main project
                for (file_path, file_model) in file_project.files {
                    project.files.insert(file_path, file_model);
                }
            }
        }

        // Create and configure tree-sitter parser for rules
        let mut ts_parser = tree_sitter::Parser::new();
        ts_parser
            .set_language(&tree_sitter_c::LANGUAGE.into())
            .expect("Failed to load C grammar");

        Ok(Self {
            project,
            ts_parser: RefCell::new(ts_parser),
        })
    }

    /// Find functions declared in headers that have no implementation
    /// Returns (file_path, function_info) tuples
    pub fn find_declared_but_not_defined(&self) -> Vec<(&Path, &FunctionInfo)> {
        self.project
            .files
            .iter()
            .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "h"))
            .flat_map(|(path, file)| {
                file.functions
                    .iter()
                    .filter(|f| !f.is_definition)
                    .filter(|f| {
                        // Check if there's a matching definition in any C file
                        !self
                            .project
                            .files
                            .iter()
                            .filter(|(p, _)| p.extension().is_some_and(|ext| ext == "c"))
                            .flat_map(|(_, file)| &file.functions)
                            .any(|def| def.name == f.name && def.is_definition)
                    })
                    .map(move |f| (path.as_path(), f))
            })
            .collect()
    }

    /// Get the source text for an entire function
    pub fn get_function_source<'a>(
        &'a self,
        file_path: &Path,
        func: &FunctionInfo,
    ) -> Option<&'a [u8]> {
        let file = self.project.files.get(file_path)?;

        if let (Some(start), Some(end)) = (func.start_byte, func.end_byte) {
            Some(&file.source[start..end])
        } else {
            None
        }
    }

    /// Iterate over all files in the project
    pub fn iter_all_files(&self) -> impl Iterator<Item = (&Path, &gobject_ast::FileModel)> {
        self.project
            .files
            .iter()
            .map(|(path, file)| (path.as_path(), file))
    }

    /// Iterate over all C files (extension .c) in the project
    pub fn iter_c_files(&self) -> impl Iterator<Item = (&Path, &gobject_ast::FileModel)> {
        self.project
            .files
            .iter()
            .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "c"))
            .map(|(path, file)| (path.as_path(), file))
    }

    /// Iterate over all header files (extension .h) in the project
    pub fn iter_header_files(&self) -> impl Iterator<Item = (&Path, &gobject_ast::FileModel)> {
        self.project
            .files
            .iter()
            .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "h"))
            .map(|(path, file)| (path.as_path(), file))
    }

    /// Parse C source code with the internal tree-sitter parser
    /// This is a convenience method for rules that need to parse function bodies
    pub fn parse_c_source(&self, source: &[u8]) -> Option<tree_sitter::Tree> {
        self.ts_parser.borrow_mut().parse(source, None)
    }

    // Common AST helper methods for rules

    /// Extract text from a tree-sitter node
    pub fn get_node_text(&self, node: tree_sitter::Node, source: &[u8]) -> String {
        let text = &source[node.byte_range()];
        std::str::from_utf8(text).unwrap_or("").to_string()
    }

    /// Find the compound_statement (function body) in an AST
    pub fn find_body<'a>(&self, node: tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
        if node.kind() == "compound_statement" {
            return Some(node);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_body(child) {
                return Some(result);
            }
        }

        None
    }

    /// Extract variable name from a declarator (handles pointer_declarator -> identifier)
    pub fn extract_variable_name(
        &self,
        declarator: tree_sitter::Node,
        source: &[u8],
    ) -> Option<String> {
        // Handle pointer_declarator -> identifier
        if let Some(inner) = declarator.child_by_field_name("declarator") {
            if inner.kind() == "identifier" {
                return Some(self.get_node_text(inner, source));
            }
            return self.extract_variable_name(inner, source);
        }

        if declarator.kind() == "identifier" {
            return Some(self.get_node_text(declarator, source));
        }

        None
    }

    /// Check if text represents a NULL literal
    pub fn is_null_literal(&self, text: &str) -> bool {
        let trimmed = text.trim();
        trimmed == "NULL" || trimmed == "0" || trimmed == "((void*)0)"
    }

    /// Find a call_expression node in the AST
    pub fn find_call_expression<'a>(
        &self,
        node: tree_sitter::Node<'a>,
    ) -> Option<tree_sitter::Node<'a>> {
        if node.kind() == "call_expression" {
            return Some(node);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_call_expression(child) {
                return Some(result);
            }
        }

        None
    }
}
