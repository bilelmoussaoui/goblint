use super::Violation;
use crate::ast_context::AstContext;
use crate::config::Config;
use tree_sitter::{Node, Parser};

pub struct GParamSpecNullNickBlurb;

impl GParamSpecNullNickBlurb {
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
            if let Some(function_node) = node.child_by_field_name("function") {
                let function_name = &source[function_node.byte_range()];
                let function_str = std::str::from_utf8(function_name).unwrap_or("");

                if function_str.starts_with("g_param_spec_")
                    && function_str != "g_param_spec_internal"
                {
                    if let Some(arguments_node) = node.child_by_field_name("arguments") {
                        let mut args = Vec::new();
                        let mut cursor = arguments_node.walk();
                        for child in arguments_node.children(&mut cursor) {
                            if child.is_named() && child.kind() != "," {
                                args.push(child);
                            }
                        }

                        if args.len() >= 3 {
                            let nick_arg = args[1];
                            let blurb_arg = args[2];

                            let mut issues = Vec::new();

                            if !self.check_argument_is_null(nick_arg, source) {
                                issues.push("nick (parameter 2)");
                            }

                            if !self.check_argument_is_null(blurb_arg, source) {
                                issues.push("blurb (parameter 3)");
                            }

                            if !issues.is_empty() {
                                violations.push(Violation {
                                    file: file_path.to_owned(),
                                    line: base_line + node.start_position().row,
                                    column: node.start_position().column + 1,
                                    message: format!(
                                        "{} should have NULL for {}",
                                        function_str,
                                        issues.join(" and ")
                                    ),
                                    rule: "g_param_spec_null_nick_blurb",
                                    snippet: None,
                                });
                            }
                        }
                    }
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(child, source, file_path, base_line, violations);
        }
    }

    fn check_argument_is_null(&self, arg_node: Node, source: &[u8]) -> bool {
        let arg_text = &source[arg_node.byte_range()];
        let arg_str = std::str::from_utf8(arg_text).unwrap_or("").trim();

        arg_str == "NULL" || arg_str == "((void*)0)" || arg_str == "0"
    }
}
