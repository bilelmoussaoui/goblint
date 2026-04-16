mod expression;
mod gobject;
mod statement;
mod top_level;

use std::{fs, path::Path};

use anyhow::{Context, Result};
use tree_sitter::{Node, Parser as TSParser};
use walkdir::WalkDir;

use crate::model::{top_level::*, *};

pub struct Parser {
    parser: TSParser,
}

impl Parser {
    pub fn new() -> Result<Self> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_c::LANGUAGE.into())
            .context("Failed to load C grammar")?;

        Ok(Self { parser })
    }

    /// Helper to create SourceLocation from a tree-sitter Node
    fn node_location(&self, node: Node) -> SourceLocation {
        SourceLocation::new(
            node.start_position().row + 1,
            node.start_position().column + 1,
            node.start_byte(),
            node.end_byte(),
        )
    }

    /// Check if a tree-sitter node is an expression
    fn is_expression_node(node: &Node) -> bool {
        matches!(
            node.kind(),
            "call_expression"
                | "assignment_expression"
                | "binary_expression"
                | "unary_expression"
                | "pointer_expression"
                | "parenthesized_expression"
                | "identifier"
                | "field_expression"
                | "string_literal"
                | "number_literal"
                | "null"
                | "NULL"
                | "true"
                | "TRUE"
                | "false"
                | "FALSE"
                | "cast_expression"
                | "conditional_expression"
                | "sizeof_expression"
                | "alignof_expression"
                | "subscript_expression"
                | "initializer_list"
                | "char_literal"
                | "update_expression"
                | "concatenated_string"
                | "compound_literal_expression"
                | "comma_expression"
                | "offsetof_expression"
                | "gnu_asm_expression"
                | "compound_statement"
                | "comment"
        )
    }

    pub fn parse_directory(&mut self, path: &Path) -> Result<Project> {
        let mut project = Project::new();

        // Parse all files (.h and .c)
        for entry in WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map_or(false, |ext| ext == "h" || ext == "c")
            })
        {
            self.parse_single_file(entry.path(), &mut project)?;
        }

        Ok(project)
    }

    pub fn parse_file(&mut self, path: &Path) -> Result<Project> {
        let mut project = Project::new();
        self.parse_single_file(path, &mut project)?;
        Ok(project)
    }

    fn parse_single_file(&mut self, path: &Path, project: &mut Project) -> Result<()> {
        let source = fs::read(path)?;
        let tree = self
            .parser
            .parse(&source, None)
            .context("Failed to parse file")?;

        let mut file_model = FileModel::new(path.to_path_buf());

        // Extract all content from this file
        self.visit_node(tree.root_node(), &source, &mut file_model);

        // Second pass: extract class structs for derivable types
        self.extract_class_structs_from_ast(
            tree.root_node(),
            &source,
            &mut file_model.gobject_types,
        );

        // Store the source for detailed pattern matching by rules
        file_model.source = source;

        project.files.insert(path.to_path_buf(), file_model);
        Ok(())
    }

    fn find_export_macros_in_declaration<'a>(
        &self,
        decl_node: Node,
        source: &'a [u8],
    ) -> Vec<&'a str> {
        let mut result = Vec::new();

        // The declaration node includes export macros when they're on the line before
        // Get the first line of the declaration
        let decl_start = decl_node.start_byte();
        let mut first_line_end = decl_start;
        while first_line_end < source.len() && source[first_line_end] != b'\n' {
            first_line_end += 1;
        }

        // Get the first line text
        if let Ok(first_line) = std::str::from_utf8(&source[decl_start..first_line_end]) {
            // Look for export macros in the first line
            for word in first_line.split_whitespace() {
                if word.ends_with("_EXPORT")
                    || word.starts_with("G_DEPRECATED")
                    || word == "G_GNUC_DEPRECATED"
                    || word == "G_GNUC_WARN_UNUSED_RESULT"
                {
                    result.push(word);
                    break; // Only take the first one
                }
            }
        }

        result
    }

    fn visit_node(&self, node: Node, source: &[u8], file_model: &mut FileModel) {
        // Try to parse as a top-level item first
        if let Some(item) = self.parse_top_level_item(node, source) {
            match item {
                TopLevelItem::Preprocessor(PreprocessorDirective::Include {
                    path,
                    is_system,
                    location,
                }) => {
                    file_model.includes.push(Include {
                        path,
                        is_system,
                        line: location.line,
                    });
                    return; // Don't recurse into includes
                }
                TopLevelItem::Preprocessor(PreprocessorDirective::Call { directive, .. }) => {
                    // Check if it's a GObject type declaration
                    if directive.starts_with("G_DECLARE_") || directive.starts_with("G_DEFINE_") {
                        if let Some(gobject_type) =
                            self.extract_gobject_type_declaration(node, source)
                        {
                            file_model.gobject_types.push(gobject_type);
                        }
                    }
                    return; // Don't recurse into preprocessor calls
                }
                TopLevelItem::Preprocessor(_) => {
                    return; // Skip other preprocessor directives
                }
                TopLevelItem::TypeDefinition(TypeDefItem::Typedef {
                    name,
                    target_type,
                    location,
                }) => {
                    file_model.typedefs.push(TypedefInfo {
                        name,
                        target_type,
                        line: location.line,
                    });

                    // Also check for typedef enums
                    if let Some(enum_info) = self.extract_enum(node, source) {
                        file_model.enums.push(enum_info);
                    }
                }
                TopLevelItem::FunctionDefinition(func_def) => {
                    // Extract parameters and other details
                    let parameters = node
                        .child_by_field_name("declarator")
                        .and_then(|d| self.find_function_declarator(d))
                        .and_then(|fd| fd.child_by_field_name("parameters"))
                        .map(|p| self.extract_parameters(p, source))
                        .unwrap_or_default();

                    let func_name = func_def.name;

                    file_model.functions.push(FunctionInfo {
                        name: func_name,
                        line: func_def.location.line,
                        is_static: func_def.is_static,
                        export_macros: Vec::new(),
                        is_definition: true,
                        return_type: None,
                        parameters,
                        start_byte: Some(func_def.location.start_byte),
                        end_byte: Some(func_def.location.end_byte),
                        body_start_byte: func_def.body_location.as_ref().map(|l| l.start_byte),
                        body_end_byte: func_def.body_location.as_ref().map(|l| l.end_byte),
                        body_statements: func_def.body_statements,
                    });

                    // Only recurse into the declarator/type, NOT into the function body
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.kind() != "compound_statement" {
                            self.visit_node(child, source, file_model);
                        }
                    }
                    return;
                }
                TopLevelItem::FunctionDeclaration(func_decl) => {
                    // Function declaration - all info already extracted in parse_top_level_item
                    if !is_macro_identifier(&func_decl.name) {
                        let func_name = func_decl.name;
                        file_model.functions.push(FunctionInfo {
                            name: func_name,
                            line: func_decl.location.line,
                            is_static: func_decl.is_static,
                            export_macros: func_decl.export_macros,
                            is_definition: false,
                            return_type: None,
                            parameters: Vec::new(),
                            start_byte: None,
                            end_byte: None,
                            body_start_byte: None,
                            body_end_byte: None,
                            body_statements: Vec::new(),
                        });
                    }
                }
                TopLevelItem::Declaration(_stmt) => {
                    // Variable/type declaration - extract structs and enums
                    if let Some(struct_info) = self.extract_struct(node, source) {
                        file_model.structs.push(struct_info);
                    }

                    if let Some(enum_info) = self.extract_enum(node, source) {
                        file_model.enums.push(enum_info);
                    }
                }
                _ => {}
            }
        }

        // Only recurse for nodes that may contain top-level items
        // (preprocessor blocks, ERROR nodes from misparsed macros)
        match node.kind() {
            "preproc_if" | "preproc_ifdef" | "preproc_elif" | "preproc_else" => {
                // Preprocessor conditionals may contain declarations
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    self.visit_node(child, source, file_model);
                }
            }
            "ERROR" => {
                // ERROR nodes from misparsed macros - look for G_DECLARE/G_DEFINE identifiers
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        let text = std::str::from_utf8(&source[child.byte_range()]).unwrap_or("");
                        if text.starts_with("G_DECLARE_") || text.starts_with("G_DEFINE_") {
                            if let Some(gobject_type) =
                                self.extract_gobject_from_identifier(child, node, source, text)
                            {
                                file_model.gobject_types.push(gobject_type);
                            }
                        }
                    }
                }
            }
            "translation_unit" => {
                // Top-level node - recurse to find all items
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    self.visit_node(child, source, file_model);
                }
            }
            "expression_statement" => {
                // Might be a GObject macro parsed as an expression
                // Recurse to find identifiers
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "call_expression" {
                        // Check if this is a G_DECLARE/G_DEFINE macro call
                        if let Some(func_node) = child.child_by_field_name("function") {
                            if func_node.kind() == "identifier" {
                                let func_name =
                                    std::str::from_utf8(&source[func_node.byte_range()])
                                        .unwrap_or("");
                                if func_name.starts_with("G_DECLARE_")
                                    || func_name.starts_with("G_DEFINE_")
                                {
                                    if let Some(gobject_type) = self
                                        .extract_gobject_from_identifier(
                                            func_node, child, source, func_name,
                                        )
                                    {
                                        file_model.gobject_types.push(gobject_type);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {
                // Other nodes are fully handled by TopLevelItem - don't recurse
            }
        }
    }

    fn extract_function_from_definition<'a>(
        &self,
        node: Node,
        source: &'a [u8],
    ) -> Option<(&'a str, bool)> {
        // Check if function definition contains "static"
        let func_text = std::str::from_utf8(&source[node.byte_range()]).ok()?;
        let is_static = func_text.starts_with("static") || func_text.contains("\nstatic ");

        let declarator = node.child_by_field_name("declarator")?;
        let name = self.extract_declarator_name(declarator, source)?;

        Some((name, is_static))
    }

    pub(super) fn find_function_declarator<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        if node.kind() == "function_declarator" {
            return Some(node);
        }

        // For pointer/abstract declarators, look in the declarator field
        if let Some(declarator) = node.child_by_field_name("declarator") {
            if let Some(found) = self.find_function_declarator(declarator) {
                return Some(found);
            }
        }

        // Recursively search children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = self.find_function_declarator(child) {
                return Some(found);
            }
        }

        None
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new().expect("Failed to create parser")
    }
}

fn is_macro_identifier(name: &str) -> bool {
    // Specific known macros and keywords
    if name == "G_DECLARE_FINAL_TYPE"
        || name == "G_DECLARE_DERIVABLE_TYPE"
        || name == "G_DECLARE_INTERFACE"
        || name == "void"
        || name == "int"
        || name.starts_with("META_TYPE_")
        || name.starts_with("CLUTTER_TYPE_")
        || name.starts_with("COGL_TYPE_")
        || name.starts_with("GTK_TYPE_")
        || name.starts_with("G_TYPE_")
        || name == "COGL_PRIVATE"
        || name.ends_with("_get_type")
        || name.ends_with("_error_quark")
        || name.ends_with("_END")
        || name == "main"
    {
        return true;
    }

    // Heuristic: if the identifier is ALL_CAPS (with underscores), it's likely a
    // macro Exception: single words like NULL, TRUE, FALSE are constants, not
    // macro calls
    if name
        .chars()
        .all(|c| c.is_uppercase() || c == '_' || c.is_numeric())
    {
        // But allow common constants/types that are legitimately all-caps
        if name == "NULL" || name == "TRUE" || name == "FALSE" {
            return false;
        }
        // If it contains an underscore or is longer than 4 chars, likely a macro
        if name.contains('_') || name.len() > 4 {
            return true;
        }
    }

    false
}
