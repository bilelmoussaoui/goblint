use super::Violation;
use crate::ast_context::AstContext;
use crate::config::Config;
use tree_sitter::Node;

pub struct StrcmpForStringEqual;

impl StrcmpForStringEqual {
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
        // Look for binary expressions like: strcmp(a, b) == 0
        if node.kind() == "binary_expression" {
            if let Some(operator) = node.child_by_field_name("operator") {
                let op_text = self.get_node_text(operator, source);

                // Only care about == and != comparisons
                if op_text == "==" || op_text == "!=" {
                    // Check left side
                    if let Some(left) = node.child_by_field_name("left") {
                        if let Some(right) = node.child_by_field_name("right") {
                            self.check_strcmp_comparison(
                                left, right, &op_text, source, file_path, base_line, node,
                                violations,
                            );
                            // Also check reverse: 0 == strcmp(a, b)
                            self.check_strcmp_comparison(
                                right, left, &op_text, source, file_path, base_line, node,
                                violations,
                            );
                        }
                    }
                }
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(child, source, file_path, base_line, violations);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn check_strcmp_comparison(
        &self,
        strcmp_side: Node,
        value_side: Node,
        operator: &str,
        source: &[u8],
        file_path: &std::path::Path,
        base_line: usize,
        parent_node: Node,
        violations: &mut Vec<Violation>,
    ) {
        // Check if strcmp_side is a call to strcmp or g_strcmp0
        if strcmp_side.kind() != "call_expression" {
            return;
        }

        let Some(function) = strcmp_side.child_by_field_name("function") else {
            return;
        };

        let func_name = self.get_node_text(function, source);
        if func_name != "strcmp" && func_name != "g_strcmp0" {
            return;
        }

        // Check if value_side is 0
        let value_text = self.get_node_text(value_side, source).trim().to_string();
        if value_text != "0" {
            return;
        }

        // Found a pattern!
        let suggestion = if operator == "==" {
            "g_str_equal"
        } else {
            "!g_str_equal"
        };

        // Extract the arguments
        if let Some(args) = strcmp_side.child_by_field_name("arguments") {
            let args_text = self.get_node_text(args, source);

            violations.push(Violation {
                file: file_path.to_owned(),
                line: base_line + parent_node.start_position().row,
                column: parent_node.start_position().column + 1,
                message: format!(
                    "Use {} {} instead of {} {} 0 for string equality",
                    suggestion, args_text, func_name, operator
                ),
                rule: "strcmp_for_string_equal",
                snippet: None,
            });
        }
    }

    fn get_node_text(&self, node: Node, source: &[u8]) -> String {
        let text = &source[node.byte_range()];
        std::str::from_utf8(text).unwrap_or("").to_string()
    }
}
