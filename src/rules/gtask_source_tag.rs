use tree_sitter::Node;

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct GTaskSourceTag;

impl Rule for GTaskSourceTag {
    fn name(&self) -> &'static str {
        "gtask_source_tag"
    }

    fn description(&self) -> &'static str {
        "Ensure g_task_set_source_tag is called after g_task_new"
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
                    let root = tree.root_node();

                    if let Some(body) = ast_context.find_body(root) {
                        let task_vars = self.find_gtask_new_calls(ast_context, body, func_source);

                        for (var_name, line_offset, col, insert_byte, indentation) in task_vars {
                            if !self.has_set_source_tag_call(
                                ast_context,
                                body,
                                &var_name,
                                func_source,
                            ) {
                                // Create fix: insert g_task_set_source_tag after the statement
                                let insert_pos = func.start_byte.unwrap() + insert_byte;
                                let fix = Fix::new(
                                    insert_pos,
                                    insert_pos,
                                    format!(
                                        "\n{}g_task_set_source_tag ({}, {});",
                                        indentation, var_name, func.name
                                    ),
                                );

                                violations.push(self.violation_with_fix(
                                    path,
                                    func.line + line_offset - 1,
                                    col,
                                    format!(
                                        "GTask '{}' created without g_task_set_source_tag",
                                        var_name
                                    ),
                                    fix,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
}

impl GTaskSourceTag {
    fn find_gtask_new_calls(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
    ) -> Vec<(String, usize, usize, usize, String)> {
        let mut results = Vec::new();

        // Look for assignments like: task = g_task_new(...)
        if node.kind() == "assignment_expression"
            && let Some(right) = node.child_by_field_name("right")
            && self.is_gtask_new_call(ast_context, right, source)
            && let Some(left) = node.child_by_field_name("left")
        {
            let var_name = ast_context.get_node_text(left, source);
            let position = node.start_position();
            // Find the parent statement to get the position after semicolon
            let insert_byte = self.find_statement_end(node);
            let indentation = self.extract_indentation(node, source);
            results.push((
                var_name,
                position.row + 1,
                position.column + 1,
                insert_byte,
                indentation,
            ));
        }

        // Look for declarations like: GTask *task = g_task_new(...)
        if node.kind() == "init_declarator"
            && let Some(value) = node.child_by_field_name("value")
            && self.is_gtask_new_call(ast_context, value, source)
            && let Some(declarator) = node.child_by_field_name("declarator")
            && let Some(var_name) = ast_context.extract_variable_name(declarator, source)
        {
            let position = node.start_position();
            // Find the parent statement to get the position after semicolon
            let insert_byte = self.find_statement_end(node);
            let indentation = self.extract_indentation(node, source);
            results.push((
                var_name,
                position.row + 1,
                position.column + 1,
                insert_byte,
                indentation,
            ));
        }

        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            results.extend(self.find_gtask_new_calls(ast_context, child, source));
        }

        results
    }

    fn extract_indentation(&self, node: Node, source: &[u8]) -> String {
        // Find the start of the line
        let mut line_start_byte = node.start_byte();

        // Walk backwards to find the start of the line
        while line_start_byte > 0 && source[line_start_byte - 1] != b'\n' {
            line_start_byte -= 1;
        }

        // Count spaces/tabs from line start to first non-whitespace
        let mut indent = String::new();
        for &byte in &source[line_start_byte..node.start_byte()] {
            if byte == b' ' || byte == b'\t' {
                indent.push(byte as char);
            } else {
                break;
            }
        }

        indent
    }

    fn find_statement_end(&self, mut node: Node) -> usize {
        // Walk up to find the statement (expression_statement or declaration)
        while let Some(parent) = node.parent() {
            if parent.kind() == "expression_statement" || parent.kind() == "declaration" {
                // Return the end byte of the statement (after the semicolon)
                return parent.end_byte();
            }
            node = parent;
        }
        // Fallback: return the end of the node itself
        node.end_byte()
    }

    fn is_gtask_new_call(&self, ast_context: &AstContext, node: Node, source: &[u8]) -> bool {
        if node.kind() != "call_expression" {
            return false;
        }

        let Some(function) = node.child_by_field_name("function") else {
            return false;
        };

        let func_text = ast_context.get_node_text(function, source);
        func_text == "g_task_new"
    }

    fn has_set_source_tag_call(
        &self,
        ast_context: &AstContext,
        node: Node,
        var_name: &str,
        source: &[u8],
    ) -> bool {
        // Look for g_task_set_source_tag(var_name, ...)
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
        {
            let func_text = ast_context.get_node_text(function, source);

            if func_text == "g_task_set_source_tag" {
                // Check if first argument matches our variable
                if let Some(arguments) = node.child_by_field_name("arguments") {
                    let args_text = ast_context.get_node_text(arguments, source);
                    // Simple check: does the arguments contain our variable name?
                    if args_text.contains(var_name) {
                        return true;
                    }
                }
            }
        }

        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if self.has_set_source_tag_call(ast_context, child, var_name, source) {
                return true;
            }
        }

        false
    }
}
