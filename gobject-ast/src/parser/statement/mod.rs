mod compound_stmt;
mod expression_stmt;
mod goto_stmt;
mod if_stmt;
mod labeled_stmt;
mod return_stmt;
mod variable_decl;

use tree_sitter::Node;

use super::Parser;
use crate::model::{CompoundStatement, Statement};

impl Parser {
    /// Parse loop statements (for, while, do-while) and switch statements as
    /// generic compound statements We don't need the loop details for most
    /// linting rules, just need to recognize them as statements so they
    /// don't get skipped
    fn parse_loop_statement(&self, node: Node, source: &[u8]) -> Option<CompoundStatement> {
        // Parse the body of the loop/switch
        let mut cursor = node.walk();
        let mut body_statements = Vec::new();

        for child in node.children(&mut cursor) {
            // Look for the loop/switch body (usually a compound_statement or single
            // statement)
            if child.kind() == "compound_statement" {
                body_statements = self.parse_function_body(child, source);
                break;
            } else if child.is_named()
                && !Parser::is_expression_node(&child)
                && child.kind() != ";"
                && child.kind() != "("
                && child.kind() != ")"
                && child.kind() != "case"
                && child.kind() != "default"
                && child.kind() != ":"
            {
                // Single statement body or case labels
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
            "for_statement" | "while_statement" | "do_statement" | "switch_statement" => {
                // Loop and switch statements - we don't need to parse them in detail for
                // linting rules, but we need to recognize them as statements so
                // they aren't silently skipped
                self.parse_loop_statement(node, source)
                    .map(Statement::Compound)
            }
            "preproc_if" | "preproc_ifdef" | "preproc_elif" | "preproc_else" => {
                // Preprocessor conditionals - parse the body statements
                self.parse_preproc_conditional(node, source)
                    .map(Statement::Compound)
            }
            "{"
            | "}"
            | ";"
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
            | "false" => {
                // Skip delimiters, comments, and loose expressions (can appear in for loop
                // clauses)
                None
            }
            "preproc_function_def"
            | "preproc_def"
            | "preproc_call"
            | "preproc_defined"
            | "preproc_include" => {
                // Preprocessor definitions - don't need to parse
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
            "break_statement" | "continue_statement" => {
                // Simple control flow statements - don't need details
                None
            }
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
                // Parse errors - skip gracefully
                None
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
}
