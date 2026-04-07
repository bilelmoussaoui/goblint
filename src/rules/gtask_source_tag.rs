use super::Violation;
use crate::ast_context::AstContext;
use crate::config::Config;
use tree_sitter::{Node, Parser};

pub struct GTaskSourceTag;

impl GTaskSourceTag {
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

    pub fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_c::LANGUAGE.into()).ok();

        for (path, file) in ast_context.project.files.iter() {
            if path.extension().is_none_or(|ext| ext != "c") {
                continue;
            }

            for func in &file.functions {
                if !func.is_definition {
                    continue;
                }

                if let Some(func_source) = ast_context.get_function_source(path, func) {
                    if let Some(tree) = parser.parse(func_source, None) {
                        let root = tree.root_node();

                        if let Some(body) = self.find_body(root) {
                            let task_vars = self.find_gtask_new_calls(body, func_source);

                            for (var_name, line_offset, col) in task_vars {
                                if !self.has_set_source_tag_call(body, &var_name, func_source) {
                                    violations.push(Violation {
                                        file: path.display().to_string(),
                                        line: func.line + line_offset - 1,
                                        column: col,
                                        message: format!(
                                            "GTask {} created without g_task_set_source_tag. Add: g_task_set_source_tag ({}, <function_name>);",
                                            var_name, var_name
                                        ),
                                        rule: "gtask_source_tag".to_string(),
                                        snippet: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn find_body<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        if node.kind() == "compound_statement" {
            return Some(node);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_body(child) {
                return Some(result);
            }
        }

        None
    }
}
