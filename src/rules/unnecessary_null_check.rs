use tree_sitter::Node;

use super::{CheckContext, Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UnnecessaryNullCheck;

impl Rule for UnnecessaryNullCheck {
    fn name(&self) -> &'static str {
        "unnecessary_null_check"
    }

    fn description(&self) -> &'static str {
        "Detect unnecessary NULL checks before g_free/g_clear_pointer"
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

                if let Some(func_source) = ast_context.get_function_source(path, func)
                    && let Some(tree) = ast_context.parse_c_source(func_source)
                {
                    let ctx = CheckContext {
                        source: func_source,
                        file_path: path,
                        base_line: func.line,
                        base_byte: func.start_byte.unwrap_or(0),
                    };
                    self.check_node(ast_context, tree.root_node(), &ctx, violations);
                }
            }
        }
    }
}

impl UnnecessaryNullCheck {
    fn check_if_statement<'a>(
        &self,
        ast_context: &AstContext,
        node: Node<'a>,
        source: &[u8],
    ) -> Option<(String, String, Node<'a>)> {
        if node.kind() != "if_statement" {
            return None;
        }

        // Get the condition
        let condition = node.child_by_field_name("condition")?;

        // Extract variable being checked (e.g., "ptr" from "ptr != NULL")
        let checked_var = self.extract_null_check_variable(ast_context, condition, source)?;

        // Get the consequence (if body)
        let consequence = node.child_by_field_name("consequence")?;

        // Check if the body contains only a g_free/g_clear_pointer call
        if let Some(free_func) =
            self.is_only_gfree_call(ast_context, consequence, &checked_var, source)
        {
            return Some((checked_var, free_func, consequence));
        }

        None
    }

    fn extract_null_check_variable(
        &self,
        ast_context: &AstContext,
        condition: Node,
        source: &[u8],
    ) -> Option<String> {
        // Look for patterns: ptr != NULL, NULL != ptr, ptr != 0, ptr

        // Handle binary expressions (ptr != NULL)
        if condition.kind() == "binary_expression" {
            if let Some(left) = condition.child_by_field_name("left") {
                let left_text = ast_context.get_node_text(left, source);
                if !ast_context.is_null_literal(&left_text) {
                    // Check operator is != or ==
                    if let Some(operator) = condition.child_by_field_name("operator") {
                        let op = ast_context.get_node_text(operator, source);
                        if op == "!=" || op == "==" {
                            return Some(left_text);
                        }
                    }
                }
            }

            // Try right side for "NULL != ptr" pattern
            if let Some(right) = condition.child_by_field_name("right") {
                let right_text = ast_context.get_node_text(right, source);
                if !ast_context.is_null_literal(&right_text) {
                    return Some(right_text);
                }
            }
        }

        // Handle simple condition (just "ptr")
        if condition.kind() == "identifier" || condition.kind() == "parenthesized_expression" {
            return Some(
                ast_context
                    .get_node_text(condition, source)
                    .trim()
                    .to_string(),
            );
        }

        None
    }

    fn is_only_gfree_call(
        &self,
        ast_context: &AstContext,
        body: Node,
        var_name: &str,
        source: &[u8],
    ) -> Option<String> {
        // For compound statements, check if it contains ONLY ONE statement total and
        // it's a g_free
        if body.kind() == "compound_statement" {
            let mut found_free = None;
            let mut total_statement_count = 0;

            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                // Count ALL statement types, not just expression_statement
                if child.kind().ends_with("_statement") || child.kind() == "declaration" {
                    total_statement_count += 1;

                    // Check if this specific statement is a g_free call
                    if child.kind() == "expression_statement"
                        && let Some(func) =
                            self.check_gfree_call(ast_context, child, var_name, source)
                    {
                        found_free = Some(func);
                    }
                }
            }

            // Only flag if there's exactly ONE statement total and it's a g_free
            if total_statement_count == 1 && found_free.is_some() {
                return found_free;
            }
        } else if body.kind() == "expression_statement" {
            // Single statement without braces
            return self.check_gfree_call(ast_context, body, var_name, source);
        }

        None
    }

    fn check_gfree_call(
        &self,
        ast_context: &AstContext,
        node: Node,
        var_name: &str,
        source: &[u8],
    ) -> Option<String> {
        // Look for g_free(var_name) or g_clear_pointer(&var_name, ...)
        if let Some(call) = ast_context.find_call_expression(node)
            && let Some(function) = call.child_by_field_name("function")
        {
            let func_name = ast_context.get_node_text(function, source);

            if func_name == "g_free" || func_name == "g_clear_pointer" {
                // Check if the argument matches our variable
                if let Some(arguments) = call.child_by_field_name("arguments") {
                    let args_text = ast_context.get_node_text(arguments, source);

                    // Simple check: does arguments contain the variable?
                    // For g_free: g_free(ptr)
                    // For g_clear_pointer: g_clear_pointer(&ptr, ...)
                    if args_text.contains(var_name) {
                        return Some(func_name);
                    }
                }
            }
        }

        None
    }

    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        if let Some((_var_name, free_func, consequence)) =
            self.check_if_statement(ast_context, node, ctx.source)
        {
            let suggestion = if free_func == "g_free" {
                "Remove unnecessary NULL check before g_free (g_free handles NULL)".to_string()
            } else {
                format!(
                    "Remove unnecessary NULL check before {} ({} handles NULL)",
                    free_func, free_func
                )
            };

            // Extract the free call statement content to replace the whole if statement
            let replacement = if consequence.kind() == "compound_statement" {
                // Get the statement inside the compound block
                let mut cursor = consequence.walk();
                let mut stmt_text = String::new();
                for child in consequence.children(&mut cursor) {
                    if child.kind() == "expression_statement" {
                        stmt_text = std::str::from_utf8(&ctx.source[child.byte_range()])
                            .unwrap_or("")
                            .to_string();
                        break;
                    }
                }
                stmt_text
            } else {
                // Single statement without braces
                std::str::from_utf8(&ctx.source[consequence.byte_range()])
                    .unwrap_or("")
                    .to_string()
            };

            let fix = Fix {
                start_byte: ctx.base_byte + node.start_byte(),
                end_byte: ctx.base_byte + node.end_byte(),
                replacement,
            };

            violations.push(self.violation_with_fix(
                ctx.file_path,
                ctx.base_line + node.start_position().row,
                node.start_position().column + 1,
                suggestion,
                fix,
            ));
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(ast_context, child, ctx, violations);
        }
    }
}
