use super::Violation;
use crate::ast_context::AstContext;
use crate::config::Config;
use tree_sitter::{Node, Parser};

pub struct UseGStrcmp0;

impl UseGStrcmp0 {
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

                        violations.push(Violation {
                            file: file_path.to_owned(),
                            line: base_line + node.start_position().row,
                            column: node.start_position().column + 1,
                            message: format!(
                                "Use {} instead of {} (NULL-safe)",
                                suggestion, func_name
                            ),
                            rule: "use_g_strcmp0",
                            snippet: None,
                        });
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
