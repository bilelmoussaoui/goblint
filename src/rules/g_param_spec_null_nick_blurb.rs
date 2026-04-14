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
                        let string_fix = if !nick_is_null && !blurb_is_null {
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

                        // Also fix the flags: after this rule runs, both nick
                        // and blurb will be NULL, so remove STATIC_NICK,
                        // STATIC_BLURB, and STATIC_STRINGS, and ensure
                        // STATIC_NAME is present (name is always a literal).
                        let mut fixes = vec![string_fix];
                        if let Some(flags_arg) = args.last()
                            && args.len() >= 4
                        {
                            let flags_text = ast_context.get_node_text(*flags_arg, ctx.source);
                            if let Some(new_flags) = self.compute_new_flags(flags_text) {
                                fixes.push(Fix::from_node(*flags_arg, ctx, new_flags));
                            }
                        }

                        violations.push(self.violation_with_fixes(
                            ctx.file_path,
                            ctx.base_line + node.start_position().row,
                            node.start_position().column + 1,
                            format!(
                                "{} should have NULL for {}",
                                function_str,
                                issues.join(" and ")
                            ),
                            fixes,
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

    /// After nick and blurb are set to NULL, compute the correct replacement
    /// flags string. Returns `None` if the flags are already correct.
    fn compute_new_flags(&self, flags_text: &str) -> Option<String> {
        const REMOVE: &[&str] = &[
            "G_PARAM_STATIC_NICK",
            "G_PARAM_STATIC_BLURB",
            "G_PARAM_STATIC_STRINGS",
        ];

        let parts: Vec<&str> = flags_text.split('|').map(|s| s.trim()).collect();
        let needs_removal = parts.iter().any(|p| REMOVE.contains(p));
        let has_name = parts.contains(&"G_PARAM_STATIC_NAME");

        if !needs_removal && has_name {
            return None;
        }

        let mut new_parts: Vec<&str> = parts.into_iter().filter(|p| !REMOVE.contains(p)).collect();

        if !new_parts.contains(&"G_PARAM_STATIC_NAME") {
            new_parts.push("G_PARAM_STATIC_NAME");
        }

        Some(new_parts.join(" | "))
    }
}
