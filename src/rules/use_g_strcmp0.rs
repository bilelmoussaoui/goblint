use super::Rule;
use crate::ast_context::AstContext;
use crate::config::Config;
use crate::rules::Violation;
use tree_sitter::Node;

pub struct UseGStrcmp0;

impl Rule for UseGStrcmp0 {
    const NAME: &'static str = "use_g_strcmp0";
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
                        self.check_node(tree.root_node(), func_source, path, func.line, violations);
                    }
                }
            }
        }
    }
}

impl UseGStrcmp0 {
    fn check_node(
        &self,
        node: Node,
        source: &[u8],
        file_path: &std::path::Path,
        base_line: usize,
        violations: &mut Vec<Violation>,
    ) {
        if node.kind() == "call_expression" {
            if let Some(function) = node.child_by_field_name("function") {
                let func_text = &source[function.byte_range()];
                if let Ok(func_name) = std::str::from_utf8(func_text) {
                    if func_name == "strcmp" || func_name == "strncmp" {
                        let suggestion = if func_name == "strcmp" {
                            "g_strcmp0"
                        } else {
                            "g_strcmp0 or check for NULL first"
                        };

                        violations.push(self.violation(
                            file_path,
                            base_line + node.start_position().row,
                            node.start_position().column + 1,
                            format!("Use {} instead of {} (NULL-safe)", suggestion, func_name),
                        ));
                    }
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(child, source, file_path, base_line, violations);
        }
    }
}
