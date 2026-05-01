mod break_stmt;
mod compound_stmt;
mod continue_stmt;
mod expression_stmt;
mod goto_stmt;
mod if_stmt;
mod labeled_stmt;
mod return_stmt;
mod switch_stmt;
mod variable_decl;

use tree_sitter::Node;

use super::Parser;
use crate::model::{CompoundStatement, Statement};

impl Parser {
    /// Parse generic statement bodies (for SEH statements, etc.)
    fn parse_loop_statement(&self, node: Node, source: &[u8]) -> Option<CompoundStatement> {
        let mut cursor = node.walk();
        let mut body_statements = Vec::new();

        for child in node.children(&mut cursor) {
            if child.kind() == "compound_statement" {
                body_statements = self.parse_function_body(child, source);
                break;
            } else if child.is_named()
                && !Parser::is_expression_node(&child)
                && child.kind() != ";"
                && child.kind() != "("
                && child.kind() != ")"
            {
                if let Some(stmt) = self.parse_statement(child, source) {
                    body_statements.push(stmt);
                }
            }
        }

        Some(CompoundStatement {
            statements: body_statements,
            location: self.node_location(node),
        })
    }

    fn parse_for_statement(
        &self,
        node: Node,
        source: &[u8],
    ) -> Option<crate::model::statement::ForStatement> {
        use crate::model::statement::ForStatement;

        let mut initializer = None;
        let mut condition = None;
        let mut update = None;
        let mut body = Vec::new();
        let mut cursor = node.walk();
        let mut semicolon_count = 0;

        for child in node.children(&mut cursor) {
            match child.kind() {
                ";" => {
                    semicolon_count += 1;
                }
                "declaration" => {
                    // Initializer is a declaration - we don't track this as an
                    // expression The declaration will be
                    // parsed separately
                }
                "compound_statement" => {
                    body = self.parse_function_body(child, source);
                }
                "(" | ")" => {
                    // Skip delimiters
                }
                _ => {
                    if Parser::is_expression_node(&child) {
                        let expr = self.parse_expression(child, source)?;
                        // Assign based on semicolon position
                        match semicolon_count {
                            0 => initializer = Some(Box::new(expr)),
                            1 => condition = Some(Box::new(expr)),
                            2 => update = Some(Box::new(expr)),
                            _ => {}
                        }
                    } else if child.is_named() {
                        // Single statement body (not in compound_statement)
                        if let Some(stmt) = self.parse_statement(child, source) {
                            body.push(stmt);
                        }
                    }
                }
            }
        }

        Some(ForStatement {
            initializer,
            condition,
            update,
            body,
            location: self.node_location(node),
        })
    }

    fn parse_while_statement(
        &self,
        node: Node,
        source: &[u8],
    ) -> Option<crate::model::statement::WhileStatement> {
        use crate::model::statement::WhileStatement;

        let mut condition = None;
        let mut body = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "compound_statement" => {
                    body = self.parse_function_body(child, source);
                }
                "(" | ")" => {
                    // Skip delimiters
                }
                _ => {
                    if Parser::is_expression_node(&child) && condition.is_none() {
                        condition = Some(Box::new(self.parse_expression(child, source)?));
                    } else if child.is_named() {
                        // Single statement body
                        if let Some(stmt) = self.parse_statement(child, source) {
                            body.push(stmt);
                        }
                    }
                }
            }
        }

        Some(WhileStatement {
            condition: condition?,
            body,
            location: self.node_location(node),
        })
    }

    fn parse_do_while_statement(
        &self,
        node: Node,
        source: &[u8],
    ) -> Option<crate::model::statement::DoWhileStatement> {
        use crate::model::statement::DoWhileStatement;

        let mut condition = None;
        let mut body = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "compound_statement" => {
                    body = self.parse_function_body(child, source);
                }
                "(" | ")" | ";" | "while" | "do" => {
                    // Skip keywords and delimiters
                }
                _ => {
                    if Parser::is_expression_node(&child) {
                        condition = Some(Box::new(self.parse_expression(child, source)?));
                    } else if child.is_named() && body.is_empty() {
                        // Single statement body (before the condition)
                        if let Some(stmt) = self.parse_statement(child, source) {
                            body.push(stmt);
                        }
                    }
                }
            }
        }

        Some(DoWhileStatement {
            body,
            condition: condition?,
            location: self.node_location(node),
        })
    }

    /// Parse preprocessor conditionals (#if, #ifdef, etc.) as compound
    /// statements
    fn parse_preproc_conditional(&self, node: Node, source: &[u8]) -> Option<CompoundStatement> {
        let mut cursor = node.walk();
        let mut body_statements = Vec::new();

        for child in node.children(&mut cursor) {
            // Skip preprocessor directives themselves (#if, #ifdef, #endif, etc.)
            if child.kind().starts_with("preproc_") && child.kind().ends_with("_directive") {
                continue;
            }

            // Parse any statements inside the preprocessor block
            if child.is_named() && child.kind() != "#" {
                if let Some(stmt) = self.parse_statement(child, source) {
                    body_statements.push(stmt);
                }
            }
        }

        Some(CompoundStatement {
            statements: body_statements,
            location: self.node_location(node),
        })
    }

    pub(super) fn parse_statement(&self, node: Node, source: &[u8]) -> Option<Statement> {
        match node.kind() {
            "declaration" => {
                // Variable declaration
                self.parse_variable_decl(node, source)
                    .map(Statement::Declaration)
            }
            "expression_statement" => {
                // Expression like function call, assignment, etc.
                self.parse_expression_stmt(node, source)
                    .map(Statement::Expression)
            }
            "if_statement" => self.parse_if_statement(node, source).map(Statement::If),
            "return_statement" => self
                .parse_return_statement(node, source)
                .map(Statement::Return),
            "goto_statement" => self.parse_goto_statement(node, source).map(Statement::Goto),
            "labeled_statement" => self
                .parse_labeled_statement(node, source)
                .map(Statement::Labeled),
            "compound_statement" => self
                .parse_compound_statement(node, source)
                .map(Statement::Compound),
            "switch_statement" => self
                .parse_switch_statement(node, source)
                .map(Statement::Switch),
            "for_statement" => self.parse_for_statement(node, source).map(Statement::For),
            "while_statement" => self
                .parse_while_statement(node, source)
                .map(Statement::While),
            "do_statement" => self
                .parse_do_while_statement(node, source)
                .map(Statement::DoWhile),
            "preproc_if" | "preproc_ifdef" | "preproc_elif" | "preproc_else" => {
                // Preprocessor conditionals - parse the body statements
                self.parse_preproc_conditional(node, source)
                    .map(Statement::Compound)
            }
            "{"
            | "}"
            | ";"
            | "("
            | ")"
            | "comment"
            | "identifier"
            | "number_literal"
            | "string_literal"
            | "char_literal"
            | "binary_expression"
            | "call_expression"
            | "unary_expression"
            | "assignment_expression"
            | "update_expression"
            | "parenthesized_expression"
            | "type_identifier"
            | "true"
            | "false"
            | "macro_modifier"
            | "storage_class_specifier"
            | "type_qualifier"
            | "primitive_type"
            | "sized_type_specifier"
            | "function_declarator"
            | "pointer_declarator"
            | "array_declarator"
            | "argument_list"
            | "if"
            | "else"
            | "while"
            | "for"
            | "do"
            | "switch"
            | "case"
            | "default"
            | "return"
            | "break"
            | "continue"
            | "goto" => {
                // Skip delimiters, comments, keywords, declaration modifiers, declarators,
                // and loose expressions (can appear in for loop clauses or as part of
                // declarations)
                None
            }
            "preproc_function_def" | "preproc_def" => {
                // Parse #define directives (function-like or object-like)
                let name_node = node.child_by_field_name("name")?;
                let name = std::str::from_utf8(&source[name_node.byte_range()])
                    .ok()?
                    .to_owned();

                // Extract value if present
                let value = node.child_by_field_name("value").and_then(|value_node| {
                    std::str::from_utf8(&source[value_node.byte_range()])
                        .ok()
                        .map(|s| s.to_owned())
                });

                Some(Statement::Preprocessor(
                    crate::model::top_level::PreprocessorDirective::Define {
                        name,
                        value,
                        location: self.node_location(node),
                    },
                ))
            }
            "preproc_call" | "preproc_defined" | "preproc_include" => {
                // Other preprocessor directives - skip for now
                None
            }
            "enum_specifier"
            | "struct_specifier"
            | "union_specifier"
            | "type_definition"
            | "macro_type_specifier" => {
                // Type definitions inside function bodies (rare but valid C)
                None
            }
            "function_definition" => {
                // Nested function definitions (GNU C extension) - skip for now
                // BUT: tree-sitter sometimes mist parses "else if" inside #ifdef as
                // function_definition In that case, try to extract the body
                // compound_statement
                if let Some(body) = node.child_by_field_name("body") {
                    if body.kind() == "compound_statement" {
                        return self
                            .parse_compound_statement(body, source)
                            .map(Statement::Compound);
                    }
                }
                None
            }
            "attributed_statement" => {
                // Statement with GNU attributes: __attribute__(...) stmt
                // Parse the underlying statement, skip the attributes
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() != "attribute_specifier" && child.is_named() {
                        return self.parse_statement(child, source);
                    }
                }
                None
            }
            "attribute_declaration" => {
                // Standalone attribute declarations (GNU C)
                None
            }
            "seh_try_statement" | "seh_except_clause" | "seh_finally_clause" => {
                // Windows SEH (Structured Exception Handling) - parse the body
                self.parse_loop_statement(node, source)
                    .map(Statement::Compound)
            }
            "break_statement" => self
                .parse_break_statement(node, source)
                .map(Statement::Break),
            "continue_statement" => self
                .parse_continue_statement(node, source)
                .map(Statement::Continue),
            "case_statement" => {
                // Case labels in switch statements - parse the body
                let mut cursor = node.walk();
                let mut statements = Vec::new();

                for child in node.children(&mut cursor) {
                    if child.is_named()
                        && child.kind() != "case"
                        && child.kind() != "default"
                        && child.kind() != ":"
                        && !child.kind().ends_with("_expression")
                    {
                        if let Some(stmt) = self.parse_statement(child, source) {
                            statements.push(stmt);
                        }
                    }
                }

                Some(Statement::Compound(CompoundStatement {
                    statements,
                    location: self.node_location(node),
                }))
            }
            "ERROR" => {
                // Parse errors - but try to extract statements from any compound_statement
                // blocks within the error node (e.g., custom loop macros)
                let mut statements = Vec::new();
                self.extract_statements_from_error_node(node, source, &mut statements);

                if statements.is_empty() {
                    None
                } else {
                    Some(Statement::Compound(CompoundStatement {
                        statements,
                        location: self.node_location(node),
                    }))
                }
            }
            _ => {
                // Unknown statement type - fail loudly so we implement it immediately
                todo!(
                    "Unimplemented statement type: {} at {}:{}",
                    node.kind(),
                    node.start_position().row + 1,
                    node.start_position().column + 1
                )
            }
        }
    }

    /// Recursively extract statements from ERROR nodes by looking for
    /// compound_statement blocks within them (e.g., from custom loop macros)
    fn extract_statements_from_error_node(
        &self,
        node: Node,
        source: &[u8],
        statements: &mut Vec<Statement>,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            // Skip keywords and punctuation that aren't statements
            if !child.is_named() {
                continue;
            }

            match child.kind() {
                "compound_statement" => {
                    // Found a block - parse it
                    if let Some(stmt) = self.parse_statement(child, source) {
                        statements.push(stmt);
                    }
                }
                "ERROR" => {
                    // Recursively search nested errors
                    self.extract_statements_from_error_node(child, source, statements);
                }
                // Skip keywords and other non-statement nodes
                "else"
                | "if"
                | "while"
                | "for"
                | "do"
                | "switch"
                | "case"
                | "default"
                | "return"
                | "break"
                | "continue"
                | "goto"
                | "static"
                | "const"
                | "volatile"
                | "register"
                | "auto"
                | "extern"
                | "typedef"
                | "struct"
                | "union"
                | "enum"
                | "sizeof"
                | "type_identifier"
                | "identifier"
                | "primitive_type"
                | "call_expression"
                | "binary_expression"
                | "unary_expression"
                | "assignment_expression"
                | "update_expression"
                | "conditional_expression"
                | "cast_expression"
                | "field_expression"
                | "subscript_expression"
                | "parenthesized_expression"
                | "number_literal"
                | "string_literal"
                | "char_literal"
                | "true"
                | "false"
                | "null"
                | "storage_class_specifier"
                | "type_qualifier"
                | "function_declarator"
                | "pointer_declarator"
                | "array_declarator"
                | "parameter_list"
                | "parameter_declaration"
                | "abstract_pointer_declarator"
                | "abstract_array_declarator"
                | "abstract_function_declarator" => {
                    // Skip these - they're not statements
                }
                _ => {
                    // Try to parse as a statement if it looks like one
                    if child.kind().ends_with("_statement") || child.kind() == "declaration" {
                        if let Some(stmt) = self.parse_statement(child, source) {
                            statements.push(stmt);
                        }
                    } else if child.has_error() {
                        // Recursively search nodes with errors
                        self.extract_statements_from_error_node(child, source, statements);
                    }
                }
            }
        }
    }
}
