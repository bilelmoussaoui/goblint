use tree_sitter::Node;

use crate::model::VariableDecl;
use crate::parser::Parser;

impl Parser {
    pub(crate) fn parse_variable_decl(&self, node: Node, source: &[u8]) -> Option<VariableDecl> {
        // declaration contains declarator and optionally type_specifier
        let mut type_name = String::new();
        let mut declarator = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "type_qualifier"
                | "storage_class_specifier"
                | "type_specifier"
                | "type_identifier"
                | "primitive_type"
                | "sized_type_specifier"
                | "struct_specifier" => {
                    if !type_name.is_empty() {
                        type_name.push(' ');
                    }
                    type_name.push_str(std::str::from_utf8(&source[child.byte_range()]).ok()?);
                }
                // Declarations with initializer: int x = 5;
                "init_declarator" => {
                    declarator = Some(child);
                }
                // Declarations without initializer: int x;  or  int *x;
                "pointer_declarator" | "identifier" | "array_declarator" => {
                    if declarator.is_none() {
                        declarator = Some(child);
                    }
                }
                _ => {}
            }
        }

        let declarator = declarator?;

        // Get variable name from declarator
        let mut var_name = None;
        let mut initializer = None;

        // For pointer types like "GError *error", check if this is a pointer declarator
        let declarator_text = std::str::from_utf8(&source[declarator.byte_range()]).ok()?;
        if declarator_text.contains('*') && !type_name.contains('*') {
            type_name.push('*');
        }

        let mut dec_cursor = declarator.walk();
        let mut has_equals = false;
        for child in declarator.children(&mut dec_cursor) {
            match child.kind() {
                "pointer_declarator" | "identifier" => {
                    // Extract identifier from declarator
                    if let Some(id) = self.find_identifier(child, source) {
                        var_name = Some(id);
                    }
                }
                "=" => {
                    has_equals = true;
                }
                _ => {
                    // Only treat as initializer if we've seen an "=" sign
                    if has_equals {
                        initializer = self.parse_expression(child, source);
                    }
                }
            }
        }

        Some(VariableDecl {
            type_name,
            name: var_name?.to_owned(),
            initializer,
            location: self.node_location(node),
        })
    }

    pub(super) fn find_identifier<'a>(&self, node: Node, source: &'a [u8]) -> Option<&'a str> {
        if node.kind() == "identifier" {
            return std::str::from_utf8(&source[node.byte_range()]).ok();
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(id) = self.find_identifier(child, source) {
                return Some(id);
            }
        }

        None
    }
}
