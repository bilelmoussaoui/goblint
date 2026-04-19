use tree_sitter::Node;

use crate::{
    model::{TypeInfo, VariableDecl},
    parser::Parser,
};

impl Parser {
    pub(crate) fn parse_variable_decl(&self, node: Node, source: &[u8]) -> Option<VariableDecl> {
        // declaration contains declarator and optionally type_specifier
        let mut type_parts = Vec::new();
        let mut is_const = false;
        let mut declarator = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "type_qualifier" => {
                    let qualifier = std::str::from_utf8(&source[child.byte_range()]).ok()?;
                    if qualifier == "const" {
                        is_const = true;
                    }
                    type_parts.push(qualifier);
                }
                "storage_class_specifier"
                | "type_specifier"
                | "type_identifier"
                | "primitive_type"
                | "sized_type_specifier"
                | "struct_specifier" => {
                    type_parts.push(std::str::from_utf8(&source[child.byte_range()]).ok()?);
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

        // Count pointer depth from declarator
        let declarator_text = std::str::from_utf8(&source[declarator.byte_range()]).ok()?;
        let pointer_depth = declarator_text.chars().filter(|&c| c == '*').count();

        let mut dec_cursor = declarator.walk();
        let mut has_equals = false;
        for child in declarator.children(&mut dec_cursor) {
            if child.kind() == "=" {
                has_equals = true;
                continue;
            }

            if !has_equals {
                // Before "=", extract variable name
                match child.kind() {
                    "pointer_declarator" | "identifier" | "array_declarator" => {
                        if let Some(id) = self.find_identifier(child, source) {
                            var_name = Some(id);
                        }
                    }
                    _ => {}
                }
            } else {
                // After "=", parse as initializer
                if child.is_named() && Parser::is_expression_node(&child) {
                    initializer = self.parse_expression(child, source);
                }
            }
        }

        // Build full type text and extract base type
        let mut full_text = type_parts.join(" ");
        if pointer_depth > 0 {
            full_text.push_str(&"*".repeat(pointer_depth));
        }

        // Extract base type (type without qualifiers or pointers)
        let base_type = type_parts
            .iter()
            .filter(|&&part| part != "const" && part != "static" && part != "extern")
            .copied()
            .collect::<Vec<_>>()
            .join(" ");

        let type_info = TypeInfo {
            base_type,
            is_const,
            pointer_depth,
            full_text,
        };

        Some(VariableDecl {
            type_info,
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
