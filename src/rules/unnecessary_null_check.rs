use super::Violation;
use crate::ast_context::AstContext;
use crate::config::Config;
use tree_sitter::Node;

pub struct UnnecessaryNullCheck;

impl UnnecessaryNullCheck {
    fn check_if_statement(&self, node: Node, source: &[u8]) -> Option<(String, String)> {
        if node.kind() != "if_statement" {
            return None;
        }

        // Get the condition
        let condition = node.child_by_field_name("condition")?;

        // Extract variable being checked (e.g., "ptr" from "ptr != NULL")
        let checked_var = self.extract_null_check_variable(condition, source)?;

        // Get the consequence (if body)
        let consequence = node.child_by_field_name("consequence")?;

        // Check if the body contains only a g_free/g_clear_pointer call
        if let Some(free_func) = self.is_only_gfree_call(consequence, &checked_var, source) {
            return Some((checked_var, free_func));
        }

        None
    }

    fn extract_null_check_variable(&self, condition: Node, source: &[u8]) -> Option<String> {
        // Look for patterns: ptr != NULL, NULL != ptr, ptr != 0, ptr

        // Handle binary expressions (ptr != NULL)
        if condition.kind() == "binary_expression" {
            if let Some(left) = condition.child_by_field_name("left") {
                let left_text = self.get_node_text(left, source);
                if !self.is_null_literal(&left_text) {
                    // Check operator is != or ==
                    if let Some(operator) = condition.child_by_field_name("operator") {
                        let op = self.get_node_text(operator, source);
                        if op == "!=" || op == "==" {
                            return Some(left_text);
                        }
                    }
                }
            }

            // Try right side for "NULL != ptr" pattern
            if let Some(right) = condition.child_by_field_name("right") {
                let right_text = self.get_node_text(right, source);
                if !self.is_null_literal(&right_text) {
                    return Some(right_text);
                }
            }
        }

        // Handle simple condition (just "ptr")
        if condition.kind() == "identifier" || condition.kind() == "parenthesized_expression" {
            return Some(self.get_node_text(condition, source).trim().to_string());
        }

        None
    }

    fn is_null_literal(&self, text: &str) -> bool {
        let trimmed = text.trim();
        trimmed == "NULL" || trimmed == "0" || trimmed == "((void*)0)"
    }

    fn is_only_gfree_call(&self, body: Node, var_name: &str, source: &[u8]) -> Option<String> {
        // For compound statements, check if it contains only one g_free call
        if body.kind() == "compound_statement" {
            let mut found_free = None;
            let mut statement_count = 0;

            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                if child.kind() == "expression_statement" {
                    statement_count += 1;
                    if let Some(func) = self.check_gfree_call(child, var_name, source) {
                        found_free = Some(func);
                    }
                }
            }

            // Only flag if there's exactly one statement and it's a g_free
            if statement_count == 1 && found_free.is_some() {
                return found_free;
            }
        } else if body.kind() == "expression_statement" {
            // Single statement without braces
            return self.check_gfree_call(body, var_name, source);
        }

        None
    }

    fn check_gfree_call(&self, node: Node, var_name: &str, source: &[u8]) -> Option<String> {
        // Look for g_free(var_name) or g_clear_pointer(&var_name, ...)
        if let Some(call) = self.find_call_expression(node) {
            if let Some(function) = call.child_by_field_name("function") {
                let func_name = self.get_node_text(function, source);

                if func_name == "g_free" || func_name == "g_clear_pointer" {
                    // Check if the argument matches our variable
                    if let Some(arguments) = call.child_by_field_name("arguments") {
                        let args_text = self.get_node_text(arguments, source);

                        // Simple check: does arguments contain the variable?
                        // For g_free: g_free(ptr)
                        // For g_clear_pointer: g_clear_pointer(&ptr, ...)
                        if args_text.contains(var_name) {
                            return Some(func_name);
                        }
                    }
                }
            }
        }

        None
    }

    fn find_call_expression<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        if node.kind() == "call_expression" {
            return Some(node);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_call_expression(child) {
                return Some(result);
            }
        }

        None
    }

    fn get_node_text(&self, node: Node, source: &[u8]) -> String {
        let text = &source[node.byte_range()];
        std::str::from_utf8(text).unwrap_or("").to_string()
    }

    pub fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        for (path, file) in ast_context.iter_c_files() {
            for func in &file.functions {
                if !func.is_definition {
                    continue;
                }

                if let Some(func_source) = ast_context.get_function_source(path, func) {
                    if let Some(tree) = ast_context.parse_c_source(func_source) {
                        self.check_node(tree.root_node(), func_source, path, func.line, violations);
                    }
                }
            }
        }
    }

    fn check_node(
        &self,
        node: Node,
        source: &[u8],
        file_path: &std::path::Path,
        base_line: usize,
        violations: &mut Vec<Violation>,
    ) {
        if let Some((_var_name, free_func)) = self.check_if_statement(node, source) {
            let suggestion = if free_func == "g_free" {
                "Remove unnecessary NULL check before g_free (g_free handles NULL)".to_string()
            } else {
                format!(
                    "Remove unnecessary NULL check before {} ({} handles NULL)",
                    free_func, free_func
                )
            };

            violations.push(Violation {
                file: file_path.to_owned(),
                line: base_line + node.start_position().row,
                column: node.start_position().column + 1,
                message: suggestion,
                rule: "unnecessary_null_check",
                snippet: None,
            });
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(child, source, file_path, base_line, violations);
        }
    }
}
