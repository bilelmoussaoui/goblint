use tree_sitter::Node;

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGClearHandleId;

impl Rule for UseGClearHandleId {
    fn name(&self) -> &'static str {
        "use_g_clear_handle_id"
    }

    fn description(&self) -> &'static str {
        "Suggest g_clear_handle_id instead of manual cleanup and zero assignment"
    }

    fn fixable(&self) -> bool {
        true
    }

    fn check_all(
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
                        let base_byte = func.start_byte.unwrap_or(0);
                        self.check_node(
                            ast_context,
                            tree.root_node(),
                            func_source,
                            path,
                            func.line,
                            base_byte,
                            violations,
                        );
                    }
                }
            }
        }
    }
}

impl UseGClearHandleId {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
        file_path: &std::path::Path,
        base_line: usize,
        base_byte: usize,
        violations: &mut Vec<Violation>,
    ) {
        // Check for if statements with g_clear_handle_id in braces that can be
        // simplified OR look for handle cleanup pattern to convert
        if node.kind() == "if_statement" {
            if let Some(consequence) = node.child_by_field_name("consequence") {
                if consequence.kind() == "compound_statement" {
                    // First check if this is the pattern to convert (g_source_remove + id = 0)
                    let conversions =
                        self.check_cleanup_then_zero(ast_context, consequence, source);

                    if !conversions.is_empty() {
                        // This is a pattern to convert
                        // Check if there are exactly 2 statements - if so, we can also remove the
                        // braces
                        let mut stmt_count = 0;
                        let mut cursor = consequence.walk();
                        for child in consequence.children(&mut cursor) {
                            if child.kind() == "expression_statement" {
                                stmt_count += 1;
                            }
                        }

                        for (var_name, cleanup_func, first_stmt, second_stmt) in conversions {
                            let position = first_stmt.start_position();
                            let replacement =
                                format!("g_clear_handle_id (&{}, {});", var_name, cleanup_func);

                            let fix = if stmt_count == 2 {
                                // Replace the entire compound_statement (including braces) with
                                // just the call
                                Fix {
                                    start_byte: base_byte + consequence.start_byte(),
                                    end_byte: base_byte + consequence.end_byte(),
                                    replacement: replacement.clone(),
                                }
                            } else {
                                // Just replace the two statements
                                Fix {
                                    start_byte: base_byte + first_stmt.start_byte(),
                                    end_byte: base_byte + second_stmt.end_byte(),
                                    replacement: replacement.clone(),
                                }
                            };

                            violations.push(self.violation_with_fix(
                                file_path,
                                base_line + position.row,
                                position.column + 1,
                                format!(
                                    "Use {} instead of {} and zero assignment",
                                    replacement, cleanup_func
                                ),
                                fix,
                            ));
                        }
                    } else {
                        // Not a conversion pattern, check if we can remove braces
                        // Count statements in the compound block
                        let mut stmt_count = 0;
                        let mut clear_handle_call = None;
                        let mut cursor = consequence.walk();
                        for child in consequence.children(&mut cursor) {
                            if child.kind() == "expression_statement" {
                                stmt_count += 1;
                                // Check if this is a g_clear_handle_id call
                                if let Some(call) = ast_context.find_call_expression(child) {
                                    if let Some(function) = call.child_by_field_name("function") {
                                        let func_name = ast_context.get_node_text(function, source);
                                        if func_name == "g_clear_handle_id" {
                                            clear_handle_call = Some(child);
                                        }
                                    }
                                }
                            }
                        }

                        // If there's exactly one statement and it's g_clear_handle_id, suggest
                        // removing braces
                        if stmt_count == 1 && clear_handle_call.is_some() {
                            let call_stmt = clear_handle_call.unwrap();

                            let position = node.start_position();
                            let fix = Fix {
                                start_byte: base_byte + consequence.start_byte(),
                                end_byte: base_byte + consequence.end_byte(),
                                replacement: std::str::from_utf8(&source[call_stmt.byte_range()])
                                    .unwrap_or("")
                                    .to_string(),
                            };

                            violations.push(self.violation_with_fix(
                                file_path,
                                base_line + position.row,
                                position.column + 1,
                                "Remove unnecessary braces around single g_clear_handle_id call".to_string(),
                                fix,
                            ));
                        }
                    }
                }
            }
        }

        // Look for compound statements (not if statements) that might have handle
        // cleanup
        if node.kind() == "compound_statement" {
            for (var_name, cleanup_func, first_stmt, second_stmt) in
                self.check_cleanup_then_zero(ast_context, node, source)
            {
                let position = first_stmt.start_position();
                let replacement = format!("g_clear_handle_id (&{}, {});", var_name, cleanup_func);

                let fix = Fix {
                    start_byte: base_byte + first_stmt.start_byte(),
                    end_byte: base_byte + second_stmt.end_byte(),
                    replacement: replacement.clone(),
                };

                violations.push(self.violation_with_fix(
                    file_path,
                    base_line + position.row,
                    position.column + 1,
                    format!(
                        "Use {} instead of {} and zero assignment",
                        replacement, cleanup_func
                    ),
                    fix,
                ));
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(
                ast_context,
                child,
                source,
                file_path,
                base_line,
                base_byte,
                violations,
            );
        }
    }

    /// Check for consecutive handle cleanup + var = 0
    /// Returns (var_name, cleanup_func, first_statement, second_statement)
    fn check_cleanup_then_zero<'a>(
        &self,
        ast_context: &AstContext,
        compound: Node<'a>,
        source: &[u8],
    ) -> Vec<(String, String, Node<'a>, Node<'a>)> {
        let mut cursor = compound.walk();
        let statements: Vec<_> = compound
            .children(&mut cursor)
            .filter(|n| n.kind() == "expression_statement")
            .collect();

        let mut results = Vec::new();

        // Look for consecutive pairs
        for i in 0..statements.len().saturating_sub(1) {
            let first = statements[i];
            let second = statements[i + 1];

            // Check if first is a handle cleanup function call
            if let Some((var_name, cleanup_func)) =
                self.extract_handle_cleanup(ast_context, first, source)
            {
                // Check if second is assignment to 0
                if let Some(assign_var) = self.extract_zero_assignment(ast_context, second, source)
                {
                    if assign_var.trim() == var_name.trim() {
                        results.push((var_name, cleanup_func, first, second));
                    }
                }
            }
        }

        results
    }

    fn extract_handle_cleanup(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
    ) -> Option<(String, String)> {
        if let Some(call) = ast_context.find_call_expression(node) {
            if let Some(function) = call.child_by_field_name("function") {
                let func_name = ast_context.get_node_text(function, source);

                // Check if this is a known handle cleanup function
                let is_handle_cleanup = matches!(
                    func_name.as_str(),
                    "g_source_remove"
                        | "g_source_destroy"
                        | "g_signal_handler_disconnect"
                        | "g_signal_handler_block"
                        | "g_signal_handler_unblock"
                );

                if !is_handle_cleanup {
                    return None;
                }

                // Get the first argument (the handle ID variable)
                if let Some(args) = call.child_by_field_name("arguments") {
                    let mut cursor = args.walk();
                    for child in args.children(&mut cursor) {
                        if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                            return Some((
                                ast_context.get_node_text(child, source).trim().to_string(),
                                func_name,
                            ));
                        }
                    }
                }
            }
        }
        None
    }

    fn extract_zero_assignment(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
    ) -> Option<String> {
        if let Some(assignment) = self.find_assignment(node) {
            if let Some(left) = assignment.child_by_field_name("left") {
                if let Some(right) = assignment.child_by_field_name("right") {
                    let right_text = ast_context.get_node_text(right, source);
                    if right_text.trim() == "0" {
                        return Some(ast_context.get_node_text(left, source).trim().to_string());
                    }
                }
            }
        }
        None
    }

    fn find_assignment<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "assignment_expression" {
                return Some(child);
            }
            if let Some(assignment) = self.find_assignment(child) {
                return Some(assignment);
            }
        }
        None
    }
}
