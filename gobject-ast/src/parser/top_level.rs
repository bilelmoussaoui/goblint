use tree_sitter::Node;

use super::Parser;
use crate::model::{top_level::*, *};

impl Parser {
    /// Find a function_declarator node within a declaration
    fn find_function_declarator_in_node<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        // Direct declarator field
        if let Some(declarator) = node.child_by_field_name("declarator") {
            if let Some(func_decl) = self.find_function_declarator(declarator) {
                return Some(func_decl);
            }
        }

        // Search all children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(func_decl) = self.find_function_declarator(child) {
                return Some(func_decl);
            }
        }

        None
    }

    /// Parse a top-level item (declaration, definition, preprocessor directive,
    /// etc.)
    pub(super) fn parse_top_level_item(&self, node: Node, source: &[u8]) -> Option<TopLevelItem> {
        match node.kind() {
            "preproc_include" => {
                let path_node = node.child_by_field_name("path")?;
                let path_text = std::str::from_utf8(&source[path_node.byte_range()]).ok()?;
                let is_system = path_text.starts_with('<');
                let path = path_text.trim_matches(&['<', '>', '"'][..]).to_owned();

                Some(TopLevelItem::Preprocessor(PreprocessorDirective::Include {
                    path,
                    is_system,
                    location: self.node_location(node),
                }))
            }
            "preproc_def" | "preproc_function_def" => {
                let name_node = node.child_by_field_name("name")?;
                let name = std::str::from_utf8(&source[name_node.byte_range()])
                    .ok()?
                    .to_owned();

                Some(TopLevelItem::Preprocessor(PreprocessorDirective::Define {
                    name,
                    location: self.node_location(node),
                }))
            }
            "preproc_call" => {
                let directive_node = node.child_by_field_name("directive")?;
                let directive = std::str::from_utf8(&source[directive_node.byte_range()])
                    .ok()?
                    .to_owned();

                Some(TopLevelItem::Preprocessor(PreprocessorDirective::Call {
                    directive,
                    location: self.node_location(node),
                }))
            }
            "preproc_if" | "preproc_ifdef" | "preproc_elif" | "preproc_else" => {
                Some(TopLevelItem::Preprocessor(PreprocessorDirective::Other {
                    location: self.node_location(node),
                }))
            }
            "type_definition" => {
                // Check for typedef
                if let Some(typedef) = self.extract_typedef_from_type_definition(node, source) {
                    return Some(TopLevelItem::TypeDefinition(TypeDefItem::Typedef {
                        name: typedef.name,
                        target_type: typedef.target_type,
                        location: self.node_location(node),
                    }));
                }
                None
            }
            "declaration" => {
                // Check if this is a function declaration
                let func_declarator = self.find_function_declarator_in_node(node);

                if let Some(func_decl) = func_declarator {
                    // Extract function name
                    let name = self.extract_declarator_name(func_decl, source)?;

                    // Check for static storage class
                    let decl_text = std::str::from_utf8(&source[node.byte_range()]).ok()?;
                    let is_static = decl_text.contains("static");

                    // Extract export macros from first line
                    let export_macros = self.find_export_macros_in_declaration(node, source);

                    return Some(TopLevelItem::FunctionDeclaration(FunctionDeclItem {
                        name: name.to_owned(),
                        is_static,
                        export_macros: export_macros.into_iter().map(|s| s.to_owned()).collect(),
                        location: self.node_location(node),
                    }));
                }

                // Variable or type declaration - parse as statement
                if let Some(stmt) = self.parse_statement(node, source) {
                    return Some(TopLevelItem::Declaration(stmt));
                }
                None
            }
            "function_definition" => {
                let (name, is_static) = self.extract_function_from_definition(node, source)?;

                // Find the function body
                let body = node.child_by_field_name("body");
                let body_statements = body
                    .map(|b| self.parse_function_body(b, source))
                    .unwrap_or_default();
                let body_location = body.map(|b| self.node_location(b));

                Some(TopLevelItem::FunctionDefinition(FunctionDefItem {
                    name: name.to_owned(),
                    is_static,
                    body_statements,
                    location: self.node_location(node),
                    body_location,
                }))
            }
            _ => None,
        }
    }

    pub(super) fn extract_typedef_from_type_definition(
        &self,
        node: Node,
        source: &[u8],
    ) -> Option<TypedefInfo> {
        // type_definition has "declarator" for the typedef name and "type" for what
        // it's typedef'ing
        let declarator_node = node.child_by_field_name("declarator")?;
        let name = std::str::from_utf8(&source[declarator_node.byte_range()]).ok()?;

        let type_node = node.child_by_field_name("type")?;
        let target_type = std::str::from_utf8(&source[type_node.byte_range()]).ok()?;

        Some(TypedefInfo {
            name: name.to_owned(),
            line: node.start_position().row + 1,
            target_type: target_type.to_owned(),
        })
    }

    pub(super) fn extract_struct(&self, node: Node, source: &[u8]) -> Option<StructInfo> {
        // Parse as declaration to use our AST types
        if let Some(Statement::Declaration(decl)) = self.parse_statement(node, source) {
            // Check if this is a struct declaration
            let decl_text =
                std::str::from_utf8(&source[decl.location.start_byte..decl.location.end_byte])
                    .ok()?;
            if decl_text.contains("struct ") {
                // Extract struct name and check for body
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "struct_specifier" {
                        if let Some(name_node) = child.child_by_field_name("name") {
                            let name = std::str::from_utf8(&source[name_node.byte_range()]).ok()?;
                            let has_body = child.child_by_field_name("body").is_some();
                            return Some(StructInfo {
                                name: name.to_owned(),
                                line: child.start_position().row + 1,
                                fields: Vec::new(), // TODO: extract fields
                                is_opaque: !has_body,
                            });
                        }
                    }
                }
            }
        }
        None
    }

    pub(super) fn extract_enum(&self, node: Node, source: &[u8]) -> Option<EnumInfo> {
        // Check if this is a typedef or regular declaration containing an enum
        let node_text = std::str::from_utf8(&source[node.byte_range()]).ok()?;
        if !node_text.contains("enum ") {
            return None;
        }

        // Handle typedef enum { ... } Name;
        if node.kind() == "type_definition" {
            if let Some(type_node) = node.child_by_field_name("type") {
                if type_node.kind() == "enum_specifier" {
                    if let Some(declarator_node) = node.child_by_field_name("declarator") {
                        let name =
                            std::str::from_utf8(&source[declarator_node.byte_range()]).ok()?;
                        if let Some(body) = type_node.child_by_field_name("body") {
                            let values = self.extract_enum_values(body, source);
                            return Some(EnumInfo {
                                name: name.to_owned(),
                                line: node.start_position().row + 1,
                                values,
                                body_start_byte: body.start_byte(),
                                body_end_byte: body.end_byte(),
                            });
                        }
                    }
                }
            }
            return None;
        }

        // Handle standalone enum Name { ... }; - parse as declaration first
        if let Some(Statement::Declaration(_)) = self.parse_statement(node, source) {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "enum_specifier" {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = std::str::from_utf8(&source[name_node.byte_range()]).ok()?;
                        if let Some(body) = child.child_by_field_name("body") {
                            let values = self.extract_enum_values(body, source);
                            return Some(EnumInfo {
                                name: name.to_owned(),
                                line: child.start_position().row + 1,
                                values,
                                body_start_byte: body.start_byte(),
                                body_end_byte: body.end_byte(),
                            });
                        }
                    }
                }
            }
        }
        None
    }

    pub(super) fn extract_enum_values(&self, body_node: Node, source: &[u8]) -> Vec<EnumValue> {
        let mut values = Vec::new();

        let mut cursor = body_node.walk();
        for child in body_node.children(&mut cursor) {
            if child.kind() == "enumerator" {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = std::str::from_utf8(&source[name_node.byte_range()])
                        .unwrap_or("")
                        .to_owned();

                    let (value, value_start, value_end) = if let Some(value_node) =
                        child.child_by_field_name("value")
                    {
                        let value_start = value_node.start_byte();
                        let value_end = value_node.end_byte();

                        // Parse as expression (only if it's actually an expression node)
                        let parsed_value = if Parser::is_expression_node(&value_node) {
                            self.parse_expression(value_node, source)
                                .and_then(|expr| match &expr {
                                    Expression::NumberLiteral(n) => n.value.parse::<i64>().ok(),
                                    Expression::Identifier(_) => None, // Symbolic constant
                                    _ => None,
                                })
                        } else {
                            None
                        };

                        (parsed_value, Some(value_start), Some(value_end))
                    } else {
                        (None, None, None)
                    };

                    values.push(EnumValue {
                        name,
                        value,
                        start_byte: child.start_byte(),
                        end_byte: child.end_byte(),
                        name_start_byte: name_node.start_byte(),
                        name_end_byte: name_node.end_byte(),
                        value_start_byte: value_start,
                        value_end_byte: value_end,
                    });
                }
            }
        }

        values
    }

    pub(super) fn extract_parameters(&self, params_node: Node, source: &[u8]) -> Vec<Parameter> {
        let mut parameters = Vec::new();

        let mut cursor = params_node.walk();
        for child in params_node.children(&mut cursor) {
            // Check node kind before processing
            if !child.is_named() || child.kind() != "parameter_declaration" {
                continue;
            }

            let type_node = child.child_by_field_name("type");
            let mut type_name = type_node
                .and_then(|t| std::str::from_utf8(&source[t.byte_range()]).ok())
                .unwrap_or_default()
                .to_owned();

            let declarator = child.child_by_field_name("declarator");
            let name = declarator
                .as_ref()
                .and_then(|d| self.extract_declarator_name(*d, source));

            // If declarator is a pointer_declarator, append * to type_name
            if let Some(decl) = declarator {
                let pointer_count = self.count_pointer_levels(decl);
                for _ in 0..pointer_count {
                    type_name.push('*');
                }
            }

            parameters.push(Parameter {
                name: name.map(ToOwned::to_owned),
                type_name,
            });
        }

        parameters
    }

    fn count_pointer_levels(&self, node: Node) -> usize {
        let mut count = 0;
        let mut current = node;

        loop {
            if current.kind() == "pointer_declarator" {
                count += 1;
                // Look for nested pointer or move to declarator field
                if let Some(inner) = current.child_by_field_name("declarator") {
                    current = inner;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        count
    }

    pub(super) fn extract_declarator_name<'a>(
        &self,
        declarator: Node,
        source: &'a [u8],
    ) -> Option<&'a str> {
        if let Some(inner) = declarator.child_by_field_name("declarator") {
            if inner.kind() == "identifier" {
                let name = &source[inner.byte_range()];
                return Some(std::str::from_utf8(name).ok()?);
            }
            return self.extract_declarator_name(inner, source);
        }

        if declarator.kind() == "identifier" {
            let name = &source[declarator.byte_range()];
            return Some(std::str::from_utf8(name).ok()?);
        }

        // Handle parenthesized declarators like (function_name) used to prevent macro
        // expansion
        if declarator.kind() == "parenthesized_declarator" {
            let mut cursor = declarator.walk();
            for child in declarator.children(&mut cursor) {
                if child.kind() == "identifier" {
                    let name = &source[child.byte_range()];
                    return Some(std::str::from_utf8(name).ok()?);
                }
            }
        }

        None
    }
}
