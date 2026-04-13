use tree_sitter::Node;

use super::{CheckContext, Fix, Rule};
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
                    let ctx = CheckContext {
                        source: func_source,
                        file_path: path,
                        base_line: func.line,
                        base_byte: func.start_byte.unwrap_or(0),
                    };
                    self.check_node(ast_context, tree.root_node(), &ctx, violations);
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
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        if node.kind() == "call_expression"
            && let Some(function_node) = node.child_by_field_name("function")
        {
            let function_str = ast_context.get_node_text(function_node, ctx.source);

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

                    let nick_is_null =
                        self.check_argument_is_null(ast_context, nick_arg, ctx.source);
                    let blurb_is_null =
                        self.check_argument_is_null(ast_context, blurb_arg, ctx.source);

                    let mut issues = Vec::new();
                    if !nick_is_null {
                        issues.push("nick (parameter 2)");
                    }
                    if !blurb_is_null {
                        issues.push("blurb (parameter 3)");
                    }

                    if !issues.is_empty() {
                        let fix = if !nick_is_null && !blurb_is_null {
                            Fix::from_range(
                                nick_arg.start_byte(),
                                blurb_arg.end_byte(),
                                ctx,
                                "NULL, NULL",
                            )
                        } else if !nick_is_null {
                            Fix::from_range(nick_arg.start_byte(), nick_arg.end_byte(), ctx, "NULL")
                        } else {
                            Fix::from_range(
                                blurb_arg.start_byte(),
                                blurb_arg.end_byte(),
                                ctx,
                                "NULL",
                            )
                        };

                        violations.push(self.violation_with_fix(
                            ctx.file_path,
                            ctx.base_line + node.start_position().row,
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
            self.check_node(ast_context, child, ctx, violations);
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
