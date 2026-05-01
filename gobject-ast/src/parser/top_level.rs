use tree_sitter::Node;

use super::Parser;
use crate::model::{top_level::*, *};

impl Parser {
    /// Extract return type from a function declaration or definition
    pub(super) fn extract_return_type(&self, node: Node, source: &[u8]) -> TypeInfo {
        let mut cursor = node.walk();
        let mut type_node = None;
        let mut is_const = false;

        // Find the type node by walking children
        // Now that grammar is fixed, macro_modifier will be a separate node we can skip
        for child in node.children(&mut cursor) {
            match child.kind() {
                "type_identifier" => {
                    if type_node.is_none() {
                        type_node = Some(child);
                    }
                }
                "primitive_type" | "sized_type_specifier" | "struct_specifier" => {
                    // Found the type
                    if type_node.is_none() {
                        type_node = Some(child);
                    }
                }
                "type_qualifier" => {
                    // Check if it's const
                    let text = std::str::from_utf8(&source[child.byte_range()]).unwrap_or("");
                    if text == "const" {
                        is_const = true;
                    }
                }
                "macro_modifier" => {
                    // Skip macro modifiers - handled by grammar now
                }
                "pointer_declarator" | "function_declarator" => {
                    // Stop when we hit the declarator
                    break;
                }
                _ => {}
            }
        }

        // Extract type text
        let (full_type_text, start_byte, end_byte) = if let Some(type_n) = type_node {
            let text = std::str::from_utf8(&source[type_n.byte_range()]).unwrap_or("void");
            (text.to_string(), type_n.start_byte(), type_n.end_byte())
        } else {
            // No type found, default to void
            (String::from("void"), node.start_byte(), node.start_byte())
        };

        let full_text = if is_const {
            format!("const {}", full_type_text)
        } else {
            full_type_text.clone()
        };

        let location = SourceLocation::new(
            node.start_position().row + 1,
            node.start_position().column + 1,
            start_byte,
            end_byte,
        );

        TypeInfo::new(full_text, location)
    }

    /// Parse a number literal string, handling both decimal and hexadecimal
    /// Returns None if the string cannot be parsed as a number
    fn parse_number_literal(literal: &str) -> Option<i64> {
        let trimmed = literal.trim();

        // Handle hex numbers (0x or 0X prefix)
        if let Some(hex_str) = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
        {
            return i64::from_str_radix(hex_str, 16).ok();
        }

        // Handle octal numbers (0 prefix, but not "0" alone)
        if trimmed.starts_with('0') && trimmed.len() > 1 && !trimmed.contains('.') {
            return i64::from_str_radix(&trimmed[1..], 8).ok();
        }

        // Handle decimal numbers
        trimmed.parse::<i64>().ok()
    }

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
    #[tracing::instrument(skip(self, node, source), fields(node_kind = node.kind(), line = node.start_position().row + 1))]
    pub(super) fn parse_top_level_item(&self, node: Node, source: &[u8]) -> Option<TopLevelItem> {
        tracing::trace!("parse_top_level_item: {}", node.kind());
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

                // Extract value if present (for #define FOO 123)
                let value = node.child_by_field_name("value").and_then(|value_node| {
                    std::str::from_utf8(&source[value_node.byte_range()])
                        .ok()
                        .map(|s| s.to_owned())
                });

                Some(TopLevelItem::Preprocessor(PreprocessorDirective::Define {
                    name,
                    value,
                    location: self.node_location(node),
                }))
            }
            "preproc_call" => {
                let directive_node = node.child_by_field_name("directive")?;
                let directive = std::str::from_utf8(&source[directive_node.byte_range()])
                    .ok()?
                    .trim_start_matches('#')
                    .to_owned();

                // Parse #pragma directives specially
                if directive == "pragma" {
                    let arguments = node.child_by_field_name("argument").and_then(|arg_node| {
                        std::str::from_utf8(&source[arg_node.byte_range()])
                            .ok()
                            .map(|s| s.trim().to_owned())
                    });

                    let kind = self.parse_pragma_kind(&arguments);

                    return Some(TopLevelItem::Preprocessor(PreprocessorDirective::Pragma {
                        kind,
                        location: self.node_location(node),
                    }));
                }

                Some(TopLevelItem::Preprocessor(PreprocessorDirective::Call {
                    directive,
                    location: self.node_location(node),
                }))
            }
            "preproc_if" | "preproc_ifdef" | "preproc_ifndef" => {
                // Parse conditional preprocessor directives with their body
                // Note: tree-sitter-c uses "preproc_ifdef" for both #ifdef and #ifndef
                // We need to check the actual text to distinguish them
                let kind = if node.kind() == "preproc_ifdef" {
                    // Check if it's actually #ifndef by looking at the directive text
                    let first_child = node.child(0);
                    let is_ifndef = first_child
                        .and_then(|child| std::str::from_utf8(&source[child.byte_range()]).ok())
                        .is_some_and(|text| text == "#ifndef");

                    if is_ifndef {
                        super::top_level::ConditionalKind::Ifndef
                    } else {
                        super::top_level::ConditionalKind::Ifdef
                    }
                } else {
                    match node.kind() {
                        "preproc_ifndef" => super::top_level::ConditionalKind::Ifndef,
                        "preproc_if" => super::top_level::ConditionalKind::If,
                        _ => unreachable!(),
                    }
                };

                // Get condition (for #ifdef/#ifndef, it's the name; for #if, it's the whole
                // condition)
                let condition = if let Some(name_node) = node.child_by_field_name("name") {
                    Some(
                        std::str::from_utf8(&source[name_node.byte_range()])
                            .ok()?
                            .to_owned(),
                    )
                } else if let Some(cond_node) = node.child_by_field_name("condition") {
                    Some(
                        std::str::from_utf8(&source[cond_node.byte_range()])
                            .ok()?
                            .to_owned(),
                    )
                } else {
                    None
                };

                // Parse body items - recursively parse children that are not part of the
                // preprocessor syntax
                let body = self.parse_conditional_body(node, source);

                Some(TopLevelItem::Preprocessor(
                    PreprocessorDirective::Conditional {
                        kind,
                        condition,
                        body,
                        location: self.node_location(node),
                    },
                ))
            }
            "preproc_elif" => {
                let condition = node
                    .child_by_field_name("condition")
                    .and_then(|c| std::str::from_utf8(&source[c.byte_range()]).ok())
                    .map(|s| s.to_owned());

                let body = self.parse_conditional_body(node, source);

                Some(TopLevelItem::Preprocessor(
                    PreprocessorDirective::Conditional {
                        kind: super::top_level::ConditionalKind::Elif,
                        condition,
                        body,
                        location: self.node_location(node),
                    },
                ))
            }
            "preproc_else" => {
                let body = self.parse_conditional_body(node, source);

                Some(TopLevelItem::Preprocessor(
                    PreprocessorDirective::Conditional {
                        kind: super::top_level::ConditionalKind::Else,
                        condition: None,
                        body,
                        location: self.node_location(node),
                    },
                ))
            }
            "gobject_decls_block" => {
                // Parse G_BEGIN_DECLS ... G_END_DECLS block
                let body = self.parse_conditional_body(node, source);

                Some(TopLevelItem::Preprocessor(
                    PreprocessorDirective::GObjectDeclsBlock {
                        body,
                        location: self.node_location(node),
                    },
                ))
            }
            "type_definition" => {
                // Check for typedef enum
                if let Some(enum_info) = self.extract_enum(node, source) {
                    return Some(TopLevelItem::TypeDefinition(TypeDefItem::Enum {
                        enum_info: Box::new(enum_info),
                    }));
                }
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
                tracing::debug!("Processing declaration node");

                // Check for enum declarations
                if let Some(enum_info) = self.extract_enum(node, source) {
                    return Some(TopLevelItem::TypeDefinition(TypeDefItem::Enum {
                        enum_info: Box::new(enum_info),
                    }));
                }

                // Check if this is a function declaration
                let func_declarator = self.find_function_declarator_in_node(node);

                if let Some(func_decl) = func_declarator {
                    // Extract function name
                    if let Some(name) = self.extract_declarator_name(func_decl, source) {
                        tracing::debug!("Found function declarator with name: {}", name);

                        // Check for static storage class
                        let decl_text = std::str::from_utf8(&source[node.byte_range()]).ok()?;
                        let is_static = decl_text.contains("static");

                        // Extract export macros from first line
                        let export_macros = self.find_export_macros_in_declaration(node, source);

                        // Extract return type
                        let return_type = self.extract_return_type(node, source);

                        return Some(TopLevelItem::FunctionDeclaration(FunctionDeclItem {
                            name: name.to_owned(),
                            return_type,
                            is_static,
                            export_macros: export_macros
                                .into_iter()
                                .map(|s| s.to_owned())
                                .collect(),
                            location: self.node_location(node),
                        }));
                    }
                }

                // Variable or type declaration - parse as statement
                if let Some(stmt) = self.parse_statement(node, source) {
                    return Some(TopLevelItem::Declaration(stmt));
                }
                None
            }
            "enum_specifier" => {
                // Standalone enum (enum Name { ... } or anonymous enum { ... })
                if let Some(enum_info) = self.extract_enum(node, source) {
                    return Some(TopLevelItem::TypeDefinition(TypeDefItem::Enum {
                        enum_info: Box::new(enum_info),
                    }));
                }
                None
            }
            "function_definition" => {
                let (name, is_static) = self.extract_function_from_definition(node, source)?;

                // Extract parameters - find parameter_list in declarator
                let parameters = if let Some(declarator) = node.child_by_field_name("declarator") {
                    // Find parameter_list recursively in the declarator tree
                    let mut params = Vec::new();
                    let mut cursor = declarator.walk();
                    for child in declarator.children_by_field_name("parameters", &mut cursor) {
                        params = self.extract_parameters(child, source);
                        break;
                    }
                    if params.is_empty() {
                        // Try finding it recursively
                        if let Some(params_node) =
                            self.find_node_by_kind(declarator, "parameter_list")
                        {
                            params = self.extract_parameters(params_node, source);
                        }
                    }
                    params
                } else {
                    Vec::new()
                };

                // Find the function body
                let body = node.child_by_field_name("body");
                let body_statements = body
                    .map(|b| self.parse_function_body(b, source))
                    .unwrap_or_default();
                let body_location = body.map(|b| self.node_location(b));

                // Extract return type
                let return_type = self.extract_return_type(node, source);

                Some(TopLevelItem::FunctionDefinition(FunctionDefItem {
                    name: name.to_owned(),
                    return_type,
                    is_static,
                    parameters,
                    body_statements,
                    location: self.node_location(node),
                    body_location,
                }))
            }
            "expression_statement" => self
                .parse_statement(node, source)
                .map(TopLevelItem::Declaration),
            "gobject_type_macro" => {
                let full_text = std::str::from_utf8(&source[node.byte_range()]).unwrap_or("");
                let macro_name = full_text.split('(').next().unwrap_or("").trim();

                // Route cleanup-func macros before the generic G_DEFINE_ handler
                if macro_name == "G_DEFINE_AUTOPTR_CLEANUP_FUNC"
                    || macro_name == "G_DEFINE_AUTO_CLEANUP_CLEAR_FUNC"
                    || macro_name == "G_DEFINE_AUTO_CLEANUP_FREE_FUNC"
                {
                    let args_node = node
                        .children(&mut node.walk())
                        .find(|c| c.kind() == "argument_list");
                    if let Some(args_node) = args_node {
                        let mut args = Vec::new();
                        self.collect_identifiers(args_node, source, &mut args);
                        if args.len() >= 2 {
                            let directive = if macro_name == "G_DEFINE_AUTOPTR_CLEANUP_FUNC" {
                                PreprocessorDirective::AutoptrCleanupFunc {
                                    type_name: args[0].to_owned(),
                                    cleanup_function: args[1].to_owned(),
                                    location: self.node_location(node),
                                }
                            } else {
                                PreprocessorDirective::AutoCleanupClearFunc {
                                    type_name: args[0].to_owned(),
                                    cleanup_function: args[1].to_owned(),
                                    location: self.node_location(node),
                                }
                            };
                            return Some(TopLevelItem::Preprocessor(directive));
                        }
                    }
                    return None;
                }

                if let Some(gobject_type) = self.extract_gobject_from_macro_modifier(node, source) {
                    return Some(TopLevelItem::Preprocessor(
                        PreprocessorDirective::GObjectType {
                            gobject_type: Box::new(gobject_type),
                            location: self.node_location(node),
                        },
                    ));
                }
                None
            }
            "ERROR" => {
                let snippet = std::str::from_utf8(&source[node.byte_range()])
                    .unwrap_or("<invalid utf8>")
                    .chars()
                    .take(80)
                    .collect::<String>();
                tracing::warn!(
                    "Unhandled ERROR node at {}:{} — fix the grammar. Content: {:?}",
                    node.start_position().row + 1,
                    node.start_position().column + 1,
                    snippet,
                );
                None
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
            location: self.node_location(node),
            target_type: target_type.to_owned(),
        })
    }

    pub(super) fn extract_enum(&self, node: Node, source: &[u8]) -> Option<EnumInfo> {
        // Check if this is a typedef or regular declaration containing an enum
        let node_text = std::str::from_utf8(&source[node.byte_range()]).ok()?;
        if !node_text.contains("enum") {
            return None;
        }

        // Handle direct enum_specifier node
        if node.kind() == "enum_specifier" {
            if let Some(body) = node.child_by_field_name("body") {
                let values = self.extract_enum_values(body, source);

                // Try to get the name from the name field
                let name = node.child_by_field_name("name").and_then(|name_node| {
                    std::str::from_utf8(&source[name_node.byte_range()])
                        .ok()
                        .map(|s| s.to_owned())
                });

                return Some(EnumInfo {
                    name,
                    location: self.node_location(node),
                    values,
                    body_location: self.node_location(body),
                    attributes: Vec::new(), // No attributes in direct enum_specifier
                });
            }
        }

        // Handle typedef enum { ... } Name;
        if node.kind() == "type_definition" {
            if let Some(type_node) = node.child_by_field_name("type") {
                if type_node.kind() == "enum_specifier" {
                    if let Some(body) = type_node.child_by_field_name("body") {
                        // Collect type_identifiers from type_definition children (these are
                        // attributes)
                        let mut attributes = Vec::new();
                        let mut cursor = node.walk();
                        for child in node.children(&mut cursor) {
                            if child.kind() == "type_identifier" {
                                if let Some(text) =
                                    std::str::from_utf8(&source[child.byte_range()]).ok()
                                {
                                    attributes.push(text.to_owned());
                                }
                            }
                        }

                        // The actual type name is the next sibling after type_definition
                        // (tree-sitter quirk)
                        let name = if let Some(next) = node.next_sibling() {
                            if next.kind() == "type_identifier" {
                                std::str::from_utf8(&source[next.byte_range()])
                                    .ok()
                                    .map(|s| s.to_owned())
                            } else {
                                // No sibling type_identifier, last child is the name
                                attributes.pop()
                            }
                        } else {
                            // No sibling, last child is the name
                            attributes.pop()
                        };

                        let values = self.extract_enum_values(body, source);
                        return Some(EnumInfo {
                            name,
                            location: self.node_location(node),
                            values,
                            body_location: self.node_location(body),
                            attributes,
                        });
                    }
                }
            }
            return None;
        }

        // Handle standalone enum Name { ... }; or anonymous enum { ... }; - parse as
        // declaration first
        if let Some(Statement::Declaration(_)) = self.parse_statement(node, source) {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "enum_specifier" {
                    if let Some(body) = child.child_by_field_name("body") {
                        let values = self.extract_enum_values(body, source);

                        // Try to get the name from the name field
                        let name = child.child_by_field_name("name").and_then(|name_node| {
                            std::str::from_utf8(&source[name_node.byte_range()])
                                .ok()
                                .map(|s| s.to_owned())
                        });

                        return Some(EnumInfo {
                            name,
                            location: self.node_location(child),
                            values,
                            body_location: self.node_location(body),
                            attributes: Vec::new(), // No attributes in standalone enum
                        });
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

                    let (value, value_expr, value_location) = if let Some(value_node) =
                        child.child_by_field_name("value")
                    {
                        // Parse as expression (only if it's actually an expression node)
                        let expr = if Parser::is_expression_node(&value_node) {
                            self.parse_expression(value_node, source)
                        } else {
                            None
                        };

                        let parsed_value = expr.as_ref().and_then(|e| match e {
                            Expression::NumberLiteral(n) => Self::parse_number_literal(&n.value),
                            Expression::Identifier(_) => None, // Symbolic constant
                            _ => None,
                        });

                        (parsed_value, expr, Some(self.node_location(value_node)))
                    } else {
                        (None, None, None)
                    };

                    values.push(EnumValue {
                        name,
                        value,
                        value_expr,
                        location: self.node_location(child),
                        name_location: self.node_location(name_node),
                        value_location,
                    });
                }
            }
        }

        values
    }

    fn find_node_by_kind<'a>(&self, node: Node<'a>, kind: &str) -> Option<Node<'a>> {
        if node.kind() == kind {
            return Some(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = self.find_node_by_kind(child, kind) {
                return Some(found);
            }
        }
        None
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
            let base_type = type_node
                .and_then(|t| std::str::from_utf8(&source[t.byte_range()]).ok())
                .unwrap_or_default()
                .to_owned();

            let declarator = child.child_by_field_name("declarator");
            let name = declarator
                .as_ref()
                .and_then(|d| self.extract_declarator_name(*d, source));

            // Count pointer levels from declarator
            let pointer_depth = if let Some(decl) = declarator {
                self.count_pointer_levels(decl)
            } else {
                0
            };

            // Build full type text
            let mut full_text = base_type;
            if pointer_depth > 0 {
                full_text.push_str(&"*".repeat(pointer_depth));
            }

            // Use type node's location if available
            let param_location = type_node
                .map(|node| self.node_location(node))
                .unwrap_or_default();
            let type_info = TypeInfo::new(full_text, param_location);

            parameters.push(Parameter {
                name: name.map(ToOwned::to_owned),
                type_info,
                location: self.node_location(child),
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

    /// Parse pragma arguments into a PragmaKind
    fn parse_pragma_kind(&self, arguments: &Option<String>) -> PragmaKind {
        let Some(args) = arguments else {
            return PragmaKind::Other {
                name: String::new(),
                arguments: None,
            };
        };

        // Check for "once"
        if args == "once" {
            return PragmaKind::Once;
        }

        // Check for diagnostic directives
        // Formats: "GCC diagnostic push", "clang diagnostic push", etc.
        if args.contains("diagnostic") {
            if args.contains("push") {
                return PragmaKind::DiagnosticPush;
            }
            if args.contains("pop") {
                return PragmaKind::DiagnosticPop;
            }
            // Check for "diagnostic ignored"
            if args.contains("ignored") {
                // Extract warning name from quotes
                // Format: "GCC diagnostic ignored \"-Wwarning-name\""
                if let Some(start) = args.find('"') {
                    if let Some(end) = args[start + 1..].find('"') {
                        let warning = args[start + 1..start + 1 + end].to_string();
                        return PragmaKind::DiagnosticIgnored { warning };
                    }
                }
            }
        }

        // Everything else goes to Other
        // Split into name and arguments
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        let name = parts[0].to_string();
        let arguments = parts.get(1).map(|s| s.to_string());

        PragmaKind::Other { name, arguments }
    }

    /// Parse the body of a conditional preprocessor block (#ifdef, #if, etc.)
    pub(super) fn parse_conditional_body(&self, node: Node, source: &[u8]) -> Vec<TopLevelItem> {
        let mut body = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            // Skip preprocessor markers (#ifdef, #endif, etc.)
            if !child.is_named()
                || matches!(
                    child.kind(),
                    "#ifdef" | "#ifndef" | "#if" | "#elif" | "#else" | "#endif"
                )
            {
                continue;
            }

            if let Some(item) = self.parse_top_level_item(child, source) {
                body.push(item);
            }
        }

        body
    }
}
