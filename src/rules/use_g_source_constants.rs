use tree_sitter::Node;

use super::{CheckContext, Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGSourceConstants;

impl Rule for UseGSourceConstants {
    fn name(&self) -> &'static str {
        "use_g_source_constants"
    }

    fn description(&self) -> &'static str {
        "Use G_SOURCE_CONTINUE/G_SOURCE_REMOVE instead of TRUE/FALSE in GSourceFunc callbacks"
    }

    fn category(&self) -> super::Category {
        super::Category::Style
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
        // Collect all callbacks passed to g_idle_add/g_timeout_add
        let mut callbacks_to_check = Vec::new();

        for (path, file) in ast_context.iter_c_files() {
            for func in &file.functions {
                if !func.is_definition {
                    continue;
                }

                if let Some(func_source) = ast_context.get_function_source(path, func)
                    && let Some(tree) = ast_context.parse_c_source(func_source)
                {
                    let root = tree.root_node();
                    if let Some(body) = ast_context.find_body(root) {
                        self.collect_source_add_callbacks(
                            ast_context,
                            body,
                            func_source,
                            &mut callbacks_to_check,
                        );
                    }
                }
            }
        }

        // Check each callback function for TRUE/FALSE returns
        for callback_name in callbacks_to_check {
            self.check_callback_returns(ast_context, callback_name, violations);
        }
    }
}

impl UseGSourceConstants {
    fn collect_source_add_callbacks<'a>(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &'a [u8],
        callbacks: &mut Vec<&'a str>,
    ) {
        // Map of source-add function name → zero-based index of the GSourceFunc
        // argument. g_idle_add(func, data)                              → 0
        // g_idle_add_full(priority, func, data, notify)       → 1
        // g_idle_add_once(func, data)                         → 0
        // g_timeout_add(interval, func, data)                 → 1
        // g_timeout_add_once(interval, func, data)            → 1
        // g_timeout_add_seconds(interval, func, data)         → 1
        // g_timeout_add_full(pri, interval, func, data, note) → 2
        // g_timeout_add_seconds_full(pri, iv, func, data, n)  → 2
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
        {
            let func_text = ast_context.get_node_text(function, source);

            let callback_arg_index: Option<usize> = match func_text {
                "g_idle_add" | "g_idle_add_once" => Some(0),
                "g_idle_add_full"
                | "g_timeout_add"
                | "g_timeout_add_once"
                | "g_timeout_add_seconds" => Some(1),
                "g_timeout_add_full" | "g_timeout_add_seconds_full" => Some(2),
                _ => None,
            };

            if let Some(arg_index) = callback_arg_index
                && let Some(arguments) = node.child_by_field_name("arguments")
                && let Some(arg) = self.get_argument_at(arguments, arg_index)
            {
                let callback_name = ast_context.get_node_text(arg, source);
                callbacks.push(callback_name);
            }
        }

        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_source_add_callbacks(ast_context, child, source, callbacks);
        }
    }

    fn get_argument_at<'a>(&self, arguments_node: Node<'a>, index: usize) -> Option<Node<'a>> {
        let mut cursor = arguments_node.walk();
        arguments_node
            .children(&mut cursor)
            .filter(|child| child.kind() != "(" && child.kind() != ")" && child.kind() != ",")
            .nth(index)
    }

    fn check_callback_returns(
        &self,
        ast_context: &AstContext,
        callback_name: &str,
        violations: &mut Vec<Violation>,
    ) {
        // Find the function definition
        for (path, file) in ast_context.iter_all_files() {
            for func in &file.functions {
                if func.name == callback_name
                    && func.is_definition
                    && let Some(func_source) = ast_context.get_function_source(path, func)
                    && let Some(tree) = ast_context.parse_c_source(func_source)
                {
                    let root = tree.root_node();
                    if let Some(body) = ast_context.find_body(root) {
                        let ctx = CheckContext {
                            source: func_source,
                            file_path: path,
                            base_line: func.line,
                            base_byte: func.start_byte.unwrap_or(0),
                        };
                        self.check_returns_in_body(ast_context, body, &ctx, violations);
                    }
                }
            }
        }
    }

    fn check_returns_in_body(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        if node.kind() == "return_statement" {
            // Get the return value
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() != "return" && child.kind() != ";" {
                    let return_value = ast_context.get_node_text(child, ctx.source);
                    let return_value_trimmed = return_value.trim();

                    if return_value_trimmed == "TRUE" || return_value_trimmed == "FALSE" {
                        let position = child.start_position();
                        let replacement = if return_value_trimmed == "TRUE" {
                            "G_SOURCE_CONTINUE"
                        } else {
                            "G_SOURCE_REMOVE"
                        };

                        let fix = Fix::from_node(child, ctx, replacement);

                        violations.push(self.violation_with_fix(
                            ctx.file_path,
                            ctx.base_line + position.row,
                            position.column + 1,
                            format!(
                                "Use {} instead of {} in GSourceFunc callback",
                                replacement, return_value_trimmed
                            ),
                            fix,
                        ));
                    }
                }
            }
        }

        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_returns_in_body(ast_context, child, ctx, violations);
        }
    }
}
