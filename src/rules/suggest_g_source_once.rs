use tree_sitter::Node;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct SuggestGSourceOnce;

impl Rule for SuggestGSourceOnce {
    fn name(&self) -> &'static str {
        "suggest_g_source_once"
    }

    fn description(&self) -> &'static str {
        "Suggest using g_idle_add_once/g_timeout_add_once when callback always returns G_SOURCE_REMOVE"
    }

    fn category(&self) -> super::Category {
        super::Category::Style
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
                    let root = tree.root_node();

                    if let Some(body) = ast_context.find_body(root) {
                        self.check_source_add_calls(
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
}

impl SuggestGSourceOnce {
    fn check_source_add_calls(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
        file_path: &std::path::Path,
        func_line: usize,
        violations: &mut Vec<Violation>,
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

                    // Find the callback function definition
                    if self.callback_always_returns_false(ast_context, &callback_name) {
                        let position = node.start_position();
                        let replacement = if func_text == "g_idle_add" {
                            "g_idle_add_once"
                        } else {
                            "g_timeout_add_once"
                        };

                        violations.push(self.violation(
                            file_path,
                            func_line + position.row,
                            position.column + 1,
                            format!(
                                "Callback '{}' always returns G_SOURCE_REMOVE. Use {} instead of {}",
                                callback_name, replacement, func_text
                            ),
                        ));
                    }
                }
            }
        }

        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_source_add_calls(
                ast_context,
                child,
                source,
                file_path,
                func_line,
                violations,
            );
        }
    }

    fn get_first_argument<'a>(&self, arguments_node: Node<'a>) -> Option<Node<'a>> {
        let mut cursor = arguments_node.walk();
        arguments_node
            .children(&mut cursor)
            .find(|&child| child.kind() != "(" && child.kind() != ")" && child.kind() != ",")
    }

    fn callback_always_returns_false(&self, ast_context: &AstContext, callback_name: &str) -> bool {
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
                        let returns = self.collect_all_returns(body, func_source, ast_context);

                        // Must have at least one return statement
                        if returns.is_empty() {
                            return false;
                        }

                        // All returns must be FALSE or G_SOURCE_REMOVE
                        return returns
                            .iter()
                            .all(|r| r == "FALSE" || r == "G_SOURCE_REMOVE" || r == "0");
                    }
                }
            }
        }
        false
    }

    fn collect_all_returns(
        &self,
        node: Node,
        source: &[u8],
        ast_context: &AstContext,
    ) -> Vec<String> {
        let mut returns = Vec::new();

        if node.kind() == "return_statement" {
            // Get the return value
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() != "return" && child.kind() != ";" {
                    let return_value = ast_context.get_node_text(child, source);
                    returns.push(return_value.trim().to_string());
                }
            }
        }

        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            returns.extend(self.collect_all_returns(child, source, ast_context));
        }

        returns
    }
}
