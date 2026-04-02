use super::{Rule, Violation};
use crate::config::Config;
use std::path::Path;
use tree_sitter::Node;

pub struct GTaskSourceTag;

impl GTaskSourceTag {
    fn check_function(&self, node: Node, source: &[u8]) -> Option<(String, usize, usize)> {
        if node.kind() != "function_definition" {
            return None;
        }

        let body = node.child_by_field_name("body")?;

        // Find g_task_new calls and their assigned variables
        let task_vars = self.find_gtask_new_calls(body, source);

        for (var_name, line, col) in task_vars {
            // Check if g_task_set_source_tag is called on this variable
            if !self.has_set_source_tag_call(body, &var_name, source) {
                return Some((var_name, line, col));
            }
        }

        None
    }

    fn find_gtask_new_calls(&self, node: Node, source: &[u8]) -> Vec<(String, usize, usize)> {
        let mut results = Vec::new();

        // Look for assignments like: task = g_task_new(...)
        if node.kind() == "assignment_expression" {
            if let Some(right) = node.child_by_field_name("right") {
                if self.is_gtask_new_call(right, source) {
                    if let Some(left) = node.child_by_field_name("left") {
                        let var_name = self.get_node_text(left, source);
                        let position = node.start_position();
                        results.push((var_name, position.row + 1, position.column + 1));
                    }
                }
            }
        }

        // Look for declarations like: GTask *task = g_task_new(...)
        if node.kind() == "init_declarator" {
            if let Some(value) = node.child_by_field_name("value") {
                if self.is_gtask_new_call(value, source) {
                    if let Some(declarator) = node.child_by_field_name("declarator") {
                        if let Some(var_name) = self.extract_variable_name(declarator, source) {
                            let position = node.start_position();
                            results.push((var_name, position.row + 1, position.column + 1));
                        }
                    }
                }
            }
        }

        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            results.extend(self.find_gtask_new_calls(child, source));
        }

        results
    }

    fn is_gtask_new_call(&self, node: Node, source: &[u8]) -> bool {
        if node.kind() != "call_expression" {
            return false;
        }

        let Some(function) = node.child_by_field_name("function") else {
            return false;
        };

        let func_text = self.get_node_text(function, source);
        func_text == "g_task_new"
    }

    fn has_set_source_tag_call(&self, node: Node, var_name: &str, source: &[u8]) -> bool {
        // Look for g_task_set_source_tag(var_name, ...)
        if node.kind() == "call_expression" {
            if let Some(function) = node.child_by_field_name("function") {
                let func_text = self.get_node_text(function, source);

                if func_text == "g_task_set_source_tag" {
                    // Check if first argument matches our variable
                    if let Some(arguments) = node.child_by_field_name("arguments") {
                        let args_text = self.get_node_text(arguments, source);
                        // Simple check: does the arguments contain our variable name?
                        if args_text.contains(var_name) {
                            return true;
                        }
                    }
                }
            }
        }

        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if self.has_set_source_tag_call(child, var_name, source) {
                return true;
            }
        }

        false
    }

    fn extract_variable_name(&self, declarator: Node, source: &[u8]) -> Option<String> {
        // Handle pointer_declarator -> identifier
        if let Some(inner) = declarator.child_by_field_name("declarator") {
            if inner.kind() == "identifier" {
                return Some(self.get_node_text(inner, source));
            }
            return self.extract_variable_name(inner, source);
        }

        if declarator.kind() == "identifier" {
            return Some(self.get_node_text(declarator, source));
        }

        None
    }

    fn get_node_text(&self, node: Node, source: &[u8]) -> String {
        let text = &source[node.byte_range()];
        std::str::from_utf8(text).unwrap_or("").to_string()
    }
}

impl Rule for GTaskSourceTag {
    fn name(&self) -> &str {
        "gtask_source_tag"
    }

    fn check(&self, node: Node, source: &[u8], file_path: &Path) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Some((var_name, line, col)) = self.check_function(node, source) {
            violations.push(Violation {
                file: file_path.display().to_string(),
                line,
                column: col,
                message: format!(
                    "GTask {} created without g_task_set_source_tag. Add: g_task_set_source_tag ({}, <function_name>);",
                    var_name, var_name
                ),
                rule: self.name().to_string(),
                snippet: None,
            });
        }

        violations
    }

    fn is_enabled(&self, config: &Config) -> bool {
        config.rules.gtask_source_tag
    }
}
