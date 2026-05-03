mod expression;
mod gobject;
mod statement;
mod top_level;

use std::{fs, path::Path};

use anyhow::{Context, Result};
use ignore::WalkBuilder;
use tree_sitter::{Node, Parser as TSParser};

use crate::model::*;

pub struct Parser {
    parser: TSParser,
    current_file: Option<std::path::PathBuf>,
}

impl Parser {
    pub fn new() -> Result<Self> {
        let mut parser = TSParser::new();
        parser
            .set_language(&tree_sitter_c_gobject::LANGUAGE.into())
            .context("Failed to load C grammar")?;

        Ok(Self {
            parser,
            current_file: None,
        })
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
        // WalkBuilder respects .gitignore by default
        for entry in WalkBuilder::new(path)
            .hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .require_git(false)
            .build()
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
        self.current_file = Some(path.to_path_buf());
        let source = fs::read(path)?;
        let tree = self
            .parser
            .parse(&source, None)
            .context("Failed to parse file")?;

        let mut file_model = FileModel::new(path.to_path_buf());

        // Extract all content from this file
        self.visit_node(tree.root_node(), &source, &mut file_model);

        // Extract all comments
        file_model.comments = self.extract_comments(tree.root_node(), &source);

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
                TopLevelItem::Preprocessor(PreprocessorDirective::Conditional { body, .. })
                | TopLevelItem::Preprocessor(PreprocessorDirective::GObjectDeclsBlock {
                    body,
                    ..
                }) => {
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

        // With the fixed grammar, macro_modifier nodes are now properly parsed
        // Just walk children and extract macro_modifier nodes
        let mut cursor = decl_node.walk();

        for child in decl_node.children(&mut cursor) {
            if child.kind() == "macro_modifier" {
                let text = std::str::from_utf8(&source[child.byte_range()]).unwrap_or("");
                result.push(text.trim());
            }
        }

        result
    }

    /// Extract GObject type from a gobject_type_macro or macro_modifier node
    /// The grammar now properly parses argument_list with identifier children
    fn extract_gobject_from_macro_modifier(
        &self,
        node: Node,
        source: &[u8],
    ) -> Option<crate::model::types::GObjectType> {
        // Collect export macros from gobject_export_macro children
        let export_macros: Vec<String> = {
            let mut cursor = node.walk();
            node.children(&mut cursor)
                .filter(|c| c.kind() == "gobject_export_macro")
                .filter_map(|c| std::str::from_utf8(&source[c.byte_range()]).ok())
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .collect()
        };

        // Get macro name from the text (before parentheses)
        let full_text = std::str::from_utf8(&source[node.byte_range()]).ok()?;
        let macro_name = full_text.split('(').next()?.trim();
        // Strip any leading export macro prefix to get the actual macro name
        let macro_name = macro_name.split_whitespace().last()?;

        let mut gobject_type =
            self.extract_gobject_from_identifier(node, node, source, macro_name)?;
        gobject_type.export_macros = export_macros;
        Some(gobject_type)
    }

    fn visit_node(&self, node: Node, source: &[u8], file_model: &mut FileModel) {
        // Try to parse this node as a top-level item. If successful, don't
        // recurse — children are handled inside parse_top_level_item itself
        // (e.g. via parse_conditional_body for #ifdef blocks).
        if let Some(item) = self.parse_top_level_item(node, source) {
            file_model.top_level_items.push(item);
            return;
        }

        // Only recurse when the node wasn't recognized as a top-level item
        // (translation_unit, unrecognized wrapper nodes, etc.)
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit_node(child, source, file_model);
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

    /// Extract all comments from the tree-sitter tree with proper position
    /// detection
    fn extract_comments(&self, root: Node, source: &[u8]) -> Vec<Comment> {
        let mut raw_comments = Vec::new();
        self.collect_comments_recursive(root, source, &mut raw_comments);

        // Determine positions by analyzing context
        self.determine_comment_positions(raw_comments, root, source)
    }

    fn collect_comments_recursive<'a>(
        &self,
        node: Node<'a>,
        source: &[u8],
        comments: &mut Vec<(Node<'a>, CommentKind, String)>,
    ) {
        // If this node is a comment, extract it
        if node.kind() == "comment" {
            if let Some((kind, content)) = self.extract_comment_text(node, source) {
                comments.push((node, kind, content));
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_comments_recursive(child, source, comments);
        }
    }

    fn extract_comment_text(&self, node: Node, source: &[u8]) -> Option<(CommentKind, String)> {
        let text = std::str::from_utf8(&source[node.byte_range()]).ok()?;

        // Determine comment kind and extract text without delimiters
        if text.starts_with("//") {
            Some((CommentKind::Line, text[2..].trim_start().to_string()))
        } else if text.starts_with("/*") && text.ends_with("*/") {
            let inner = &text[2..text.len() - 2];
            Some((CommentKind::Block, inner.trim().to_string()))
        } else {
            None
        }
    }

    fn determine_comment_positions(
        &self,
        raw_comments: Vec<(Node, CommentKind, String)>,
        root: Node,
        source: &[u8],
    ) -> Vec<Comment> {
        raw_comments
            .into_iter()
            .map(|(node, kind, content)| {
                let location = self.node_location(node);
                let position = self.detect_comment_position(node, root, source);
                Comment::new(content, location, kind, position)
            })
            .collect()
    }

    fn detect_comment_position(
        &self,
        comment_node: Node,
        root: Node,
        _source: &[u8],
    ) -> CommentPosition {
        let comment_line = comment_node.start_position().row;
        let comment_end_line = comment_node.end_position().row;

        // Find the next non-comment node after this comment
        if let Some(next_node) = self.find_next_sibling_or_statement(comment_node, root) {
            let next_line = next_node.start_position().row;

            // If comment is on the same line as the next node, it's trailing
            if comment_line == next_line || comment_end_line == next_line {
                return CommentPosition::Trailing;
            }
        }

        // Find the previous non-comment node before this comment
        if let Some(prev_node) = self.find_prev_sibling_or_statement(comment_node, root) {
            let prev_end_line = prev_node.end_position().row;

            // If comment is on the same line as previous node's end, it's trailing
            if comment_line == prev_end_line {
                return CommentPosition::Trailing;
            }
        }

        // Default to Leading for comments before statements
        CommentPosition::Leading
    }

    fn find_next_sibling_or_statement<'a>(&self, node: Node<'a>, _root: Node) -> Option<Node<'a>> {
        let mut current = node;
        loop {
            if let Some(sibling) = current.next_sibling() {
                if sibling.kind() != "comment" {
                    return Some(sibling);
                }
                current = sibling;
            } else {
                return None;
            }
        }
    }

    fn find_prev_sibling_or_statement<'a>(&self, node: Node<'a>, _root: Node) -> Option<Node<'a>> {
        let mut current = node;
        loop {
            if let Some(sibling) = current.prev_sibling() {
                if sibling.kind() != "comment" {
                    return Some(sibling);
                }
                current = sibling;
            } else {
                return None;
            }
        }
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new().expect("Failed to create parser")
    }
}
