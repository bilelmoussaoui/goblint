use tree_sitter::Node;

use crate::{
    model::{Expression, SourceLocation, TypeInfo, VariableDecl},
    parser::Parser,
};

impl Parser {
    pub(crate) fn parse_variable_decl(&self, node: Node, source: &[u8]) -> Option<VariableDecl> {
        // declaration contains declarator and optionally type_specifier
        let mut type_parts = Vec::new();
        let mut declarator = None;
        let mut first_type_node: Option<Node> = None;
        let mut last_type_node: Option<Node> = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "type_qualifier" => {
                    let qualifier = std::str::from_utf8(&source[child.byte_range()]).ok()?;
                    type_parts.push(qualifier);
                    if first_type_node.is_none() {
                        first_type_node = Some(child);
                    }
                    last_type_node = Some(child);
                }
                "storage_class_specifier"
                | "type_specifier"
                | "type_identifier"
                | "primitive_type"
                | "sized_type_specifier"
                | "struct_specifier" => {
                    type_parts.push(std::str::from_utf8(&source[child.byte_range()]).ok()?);
                    if first_type_node.is_none() {
                        first_type_node = Some(child);
                    }
                    last_type_node = Some(child);
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

        // Extract array size from declarator (searches recursively for
        // array_declarator)
        let array_size = self.extract_array_size(declarator, source);

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

        // Build full type text
        let mut full_text = type_parts.join(" ");
        if pointer_depth > 0 {
            full_text.push_str(&"*".repeat(pointer_depth));
        }

        // TypeInfo::new() will automatically filter out storage class specifiers
        let type_location = if let (Some(first), Some(last)) = (first_type_node, last_type_node) {
            SourceLocation::new(
                first.start_position().row + 1,
                first.start_position().column,
                first.start_byte(),
                last.end_byte(),
            )
        } else {
            SourceLocation::default()
        };
        let type_info = TypeInfo::new(full_text, type_location);

        Some(VariableDecl {
            type_info,
            name: var_name?.to_owned(),
            initializer,
            array_size,
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

    /// Extract array size expression from a declarator (recursively searches
    /// for array_declarator) e.g., for "int arr[N_PROPS]", extracts N_PROPS
    /// as an expression
    pub(super) fn extract_array_size(&self, declarator: Node, source: &[u8]) -> Option<Expression> {
        // Recursively find array_declarator and extract its size
        self.find_array_size_recursive(declarator, source)
    }

    fn find_array_size_recursive(&self, node: Node, source: &[u8]) -> Option<Expression> {
        if node.kind() == "array_declarator" {
            let mut cursor = node.walk();
            let mut found_bracket = false;
            for child in node.children(&mut cursor) {
                // Skip everything until we find "["
                if child.kind() == "[" {
                    found_bracket = true;
                    continue;
                }
                // Stop at "]"
                if child.kind() == "]" {
                    break;
                }
                // After "[", look for the size expression
                if found_bracket && child.is_named() && Parser::is_expression_node(&child) {
                    return self.parse_expression(child, source);
                }
            }
            return None;
        }

        // Recursively search children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(size) = self.find_array_size_recursive(child, source) {
                return Some(size);
            }
        }

        None
    }
}
