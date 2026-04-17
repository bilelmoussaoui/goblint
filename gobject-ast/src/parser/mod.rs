mod expression;
mod gobject;
mod statement;
mod top_level;

use std::{fs, path::Path};

use anyhow::{Context, Result};
use tree_sitter::{Node, Parser as TSParser};
use walkdir::WalkDir;

use crate::model::*;

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

        // Store the source for detailed pattern matching by rules
        file_model.source = source.clone();

        // Post-processing: populate class_struct fields on GObjectType items
        self.populate_class_structs(tree.root_node(), &source, &mut file_model);

        project.files.insert(path.to_path_buf(), file_model);
        Ok(())
    }

    fn populate_class_structs(&self, root: Node, source: &[u8], file_model: &mut FileModel) {
        // Collect all GObjectType items (mutable references)
        let mut gobject_types = Vec::new();
        self.collect_gobject_types_mut(&mut file_model.top_level_items, &mut gobject_types);

        // Extract class structs for each GObjectType
        if !gobject_types.is_empty() {
            self.extract_class_structs_from_ast(root, source, &mut gobject_types);
        }
    }

    fn collect_gobject_types_mut<'a>(
        &self,
        items: &'a mut [crate::top_level::TopLevelItem],
        gobject_types: &mut Vec<&'a mut crate::model::types::GObjectType>,
    ) {
        use crate::top_level::{PreprocessorDirective, TopLevelItem};

        for item in items {
            match item {
                TopLevelItem::Preprocessor(PreprocessorDirective::GObjectType {
                    gobject_type,
                    ..
                }) => {
                    gobject_types.push(gobject_type.as_mut());
                }
                TopLevelItem::Preprocessor(PreprocessorDirective::Conditional { body, .. }) => {
                    self.collect_gobject_types_mut(body, gobject_types);
                }
                _ => {}
            }
        }
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
            // Simply push the item - the tree structure is already built
            file_model.top_level_items.push(item);
            return;
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
                                use crate::top_level::{PreprocessorDirective, TopLevelItem};
                                let location = self.node_location(node);
                                file_model.top_level_items.push(TopLevelItem::Preprocessor(
                                    PreprocessorDirective::GObjectType {
                                        gobject_type: Box::new(gobject_type),
                                        location,
                                    },
                                ));
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
                                        use crate::top_level::{
                                            PreprocessorDirective, TopLevelItem,
                                        };
                                        let location = self.node_location(node);
                                        file_model.top_level_items.push(
                                            TopLevelItem::Preprocessor(
                                                PreprocessorDirective::GObjectType {
                                                    gobject_type: Box::new(gobject_type),
                                                    location,
                                                },
                                            ),
                                        );
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
