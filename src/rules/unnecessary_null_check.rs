use tree_sitter::Node;

use super::{CheckContext, Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UnnecessaryNullCheck;

impl Rule for UnnecessaryNullCheck {
    fn name(&self) -> &'static str {
        "unnecessary_null_check"
    }

    fn description(&self) -> &'static str {
        "Detect unnecessary NULL checks before g_free/g_clear_* functions"
    }

    fn category(&self) -> super::Category {
        super::Category::Suspicious
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
        source: &'a [u8],
    ) -> Option<(&'a str, &'a str, Node<'a>)> {
        if node.kind() != "if_statement" {
            return None;
        }

        // Don't flag if there's an else branch — removing the if would also
        // drop the else logic.
        if node.child_by_field_name("alternative").is_some() {
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
            self.is_only_gfree_call(ast_context, consequence, checked_var, source)
        {
            return Some((checked_var, free_func, consequence));
        }

        None
    }

    fn extract_null_check_variable<'a>(
        &self,
        ast_context: &AstContext,
        condition: Node,
        source: &'a [u8],
    ) -> Option<&'a str> {
        // Look for patterns: ptr != NULL, NULL != ptr, ptr != 0, ptr

        // Handle binary expressions (ptr != NULL)
        if condition.kind() == "binary_expression" {
            if let Some(left) = condition.child_by_field_name("left") {
                let left_text = ast_context.get_node_text(left, source);
                if !ast_context.is_null_literal(left_text) {
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
                if !ast_context.is_null_literal(right_text) {
                    return Some(right_text);
                }
            }
        }

        // Handle parenthesized expression - recurse into the inner expression
        if condition.kind() == "parenthesized_expression" {
            let mut cursor = condition.walk();
            for child in condition.children(&mut cursor) {
                if child.kind() != "(" && child.kind() != ")" {
                    return self.extract_null_check_variable(ast_context, child, source);
                }
            }
        }

        // Handle simple identifier condition (just "ptr")
        if condition.kind() == "identifier" {
            return Some(ast_context.get_node_text(condition, source).trim());
        }

        None
    }

    fn is_only_gfree_call<'a>(
        &self,
        ast_context: &AstContext,
        body: Node,
        var_name: &str,
        source: &'a [u8],
    ) -> Option<&'a str> {
        // For compound statements, check if it contains ONLY ONE statement total and
        // it's a g_free or g_clear_*
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

    fn check_gfree_call<'a>(
        &self,
        ast_context: &AstContext,
        node: Node,
        var_name: &str,
        source: &'a [u8],
    ) -> Option<&'a str> {
        // Look for g_free(var_name) or g_clear_*(&var_name, ...)
        if let Some(call) = ast_context.find_call_expression(node)
            && let Some(function) = call.child_by_field_name("function")
        {
            let func_name = ast_context.get_node_text(function, source);

            // Check for g_free or any g_clear_* function
            if func_name == "g_free" || func_name.starts_with("g_clear_") {
                // Check if the argument matches our variable
                if let Some(arguments) = call.child_by_field_name("arguments") {
                    let args_text = ast_context.get_node_text(arguments, source);

                    // Simple check: does arguments contain the variable?
                    // For g_free: g_free(ptr)
                    // For g_clear_*: g_clear_object(&ptr), g_clear_pointer(&ptr, ...), etc.
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
            let suggestion = format!(
                "Remove unnecessary NULL check before {} ({} handles NULL)",
                free_func, free_func
            );

            // Extract the free call statement content to replace the whole if statement
            let replacement = if consequence.kind() == "compound_statement" {
                // Get the statement inside the compound block
                let mut cursor = consequence.walk();
                let mut stmt_text = "";
                for child in consequence.children(&mut cursor) {
                    if child.kind() == "expression_statement" {
                        stmt_text = ast_context.get_node_text(child, ctx.source);
                        break;
                    }
                }
                stmt_text
            } else {
                // Single statement without braces
                ast_context.get_node_text(consequence, ctx.source)
            };

            let fix = Fix::from_node(node, ctx, replacement);

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
