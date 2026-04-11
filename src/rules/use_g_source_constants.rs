use tree_sitter::Node;

use super::{Fix, Rule};
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
            self.check_callback_returns(ast_context, &callback_name, violations);
        }
    }
}

impl UseGSourceConstants {
    fn collect_source_add_callbacks(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
        callbacks: &mut Vec<String>,
    ) {
        // Look for g_idle_add or g_timeout_add calls
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
        {
            let func_text = ast_context.get_node_text(function, source);

            if func_text == "g_idle_add" || func_text == "g_timeout_add" {
                // Get the first argument (the callback function)
                if let Some(arguments) = node.child_by_field_name("arguments")
                    && let Some(first_arg) = self.get_first_argument(arguments)
                {
                    let callback_name = ast_context.get_node_text(first_arg, source);
                    callbacks.push(callback_name);
                }
            }
        }

        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_source_add_callbacks(ast_context, child, source, callbacks);
        }
    }

    fn get_first_argument<'a>(&self, arguments_node: Node<'a>) -> Option<Node<'a>> {
        let mut cursor = arguments_node.walk();
        arguments_node
            .children(&mut cursor)
            .find(|&child| child.kind() != "(" && child.kind() != ")" && child.kind() != ",")
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
                        self.check_returns_in_body(
                            ast_context,
                            body,
                            func_source,
                            path,
                            func.line,
                            violations,
                        );
                    }
                }
            }
        }
    }

    fn check_returns_in_body(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
        file_path: &std::path::Path,
        func_line: usize,
        violations: &mut Vec<Violation>,
    ) {
        if node.kind() == "return_statement" {
            // Get the return value
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() != "return" && child.kind() != ";" {
                    let return_value = ast_context.get_node_text(child, source);
                    let return_value_trimmed = return_value.trim();

                    if return_value_trimmed == "TRUE" || return_value_trimmed == "FALSE" {
                        let position = child.start_position();
                        let replacement = if return_value_trimmed == "TRUE" {
                            "G_SOURCE_CONTINUE"
                        } else {
                            "G_SOURCE_REMOVE"
                        };

                        let fix = Fix::new(child.start_byte(), child.end_byte(), replacement);

                        violations.push(self.violation_with_fix(
                            file_path,
                            func_line + position.row,
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
            self.check_returns_in_body(
                ast_context,
                child,
                source,
                file_path,
                func_line,
                violations,
            );
        }
    }
}
