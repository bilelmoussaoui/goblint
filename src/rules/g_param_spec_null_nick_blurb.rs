use tree_sitter::Node;

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct GParamSpecNullNickBlurb;

impl Rule for GParamSpecNullNickBlurb {
    fn name(&self) -> &'static str {
        "g_param_spec_null_nick_blurb"
    }

    fn description(&self) -> &'static str {
        "Ensure g_param_spec_* functions have NULL for nick and blurb parameters"
    }

    fn category(&self) -> super::Category {
        super::Category::Pedantic
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
                    let base_byte = func.start_byte.unwrap_or(0);
                    self.check_node(
                        ast_context,
                        tree.root_node(),
                        func_source,
                        path,
                        func.line,
                        base_byte,
                        violations,
                    );
                }
            }
        }
    }
}

impl GParamSpecNullNickBlurb {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
        file_path: &std::path::Path,
        base_line: usize,
        base_byte: usize,
        violations: &mut Vec<Violation>,
    ) {
        if node.kind() == "call_expression"
            && let Some(function_node) = node.child_by_field_name("function")
        {
            let function_str = ast_context.get_node_text(function_node, source);

            if function_str.starts_with("g_param_spec_")
                && function_str != "g_param_spec_internal"
                && let Some(arguments_node) = node.child_by_field_name("arguments")
            {
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

                    let nick_is_null = self.check_argument_is_null(ast_context, nick_arg, source);
                    let blurb_is_null = self.check_argument_is_null(ast_context, blurb_arg, source);

                    let mut issues = Vec::new();
                    if !nick_is_null {
                        issues.push("nick (parameter 2)");
                    }
                    if !blurb_is_null {
                        issues.push("blurb (parameter 3)");
                    }

                    if !issues.is_empty() {
                        // If both need fixing, replace both in a single fix
                        // If only one needs fixing, replace just that one
                        let (start, end, replacement) = if !nick_is_null && !blurb_is_null {
                            // Replace both: from start of nick to end of blurb
                            (
                                base_byte + nick_arg.start_byte(),
                                base_byte + blurb_arg.end_byte(),
                                "NULL, NULL".to_string(),
                            )
                        } else if !nick_is_null {
                            // Replace only nick
                            (
                                base_byte + nick_arg.start_byte(),
                                base_byte + nick_arg.end_byte(),
                                "NULL".to_string(),
                            )
                        } else {
                            // Replace only blurb
                            (
                                base_byte + blurb_arg.start_byte(),
                                base_byte + blurb_arg.end_byte(),
                                "NULL".to_string(),
                            )
                        };

                        let fix = Fix::new(start, end, replacement);

                        violations.push(self.violation_with_fix(
                            file_path,
                            base_line + node.start_position().row,
                            node.start_position().column + 1,
                            format!(
                                "{} should have NULL for {}",
                                function_str,
                                issues.join(" and ")
                            ),
                            fix,
                        ));
                    }
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(
                ast_context,
                child,
                source,
                file_path,
                base_line,
                base_byte,
                violations,
            );
        }
    }

    fn check_argument_is_null(
        &self,
        ast_context: &AstContext,
        arg_node: Node,
        source: &[u8],
    ) -> bool {
        let arg_str = ast_context.get_node_text(arg_node, source);
        let arg_str = arg_str.trim();

        arg_str == "NULL" || arg_str == "((void*)0)" || arg_str == "0"
    }
}
