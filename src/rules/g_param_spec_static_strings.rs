use tree_sitter::Node;

use super::{CheckContext, Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct GParamSpecStaticStrings;

impl Rule for GParamSpecStaticStrings {
    fn name(&self) -> &'static str {
        "g_param_spec_static_strings"
    }

    fn description(&self) -> &'static str {
        "Ensure g_param_spec_* calls use G_PARAM_STATIC_STRINGS flag for string literals"
    }

    fn category(&self) -> super::Category {
        super::Category::Perf
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

impl GParamSpecStaticStrings {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        // Look for g_param_spec_* calls
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
        {
            let func_name = ast_context.get_node_text(function, ctx.source);

            if func_name.starts_with("g_param_spec_")
                && func_name != "g_param_spec_override"
                && func_name != "g_param_spec_internal"
            {
                // Check if this g_param_spec call has string literals and missing
                // G_PARAM_STATIC_STRINGS
                if let Some((flags_arg, flags_arg_text, has_static_strings)) =
                    self.check_param_spec_flags(ast_context, node, ctx.source)
                    && !has_static_strings
                {
                    let fix = if flags_arg_text.is_empty() || flags_arg_text == "0" {
                        Fix::from_node(flags_arg, ctx, "G_PARAM_STATIC_STRINGS")
                    } else {
                        Fix::from_node(
                            flags_arg,
                            ctx,
                            format!("{} | G_PARAM_STATIC_STRINGS", flags_arg_text),
                        )
                    };

                    violations.push(self.violation_with_fix(
                            ctx.file_path,
                            ctx.base_line + node.start_position().row,
                            node.start_position().column + 1,
                            format!(
                                "Add G_PARAM_STATIC_STRINGS to {} flags (saves memory for static strings)",
                                func_name
                            ),
                            fix,
                        ));
                }
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(ast_context, child, ctx, violations);
        }
    }

    /// Check if g_param_spec_* has string literals and whether it has
    /// G_PARAM_STATIC_STRINGS Returns (flags_arg_node, flags_text,
    /// has_static_strings)
    fn check_param_spec_flags<'a>(
        &self,
        ast_context: &AstContext,
        call_node: Node<'a>,
        source: &'a [u8],
    ) -> Option<(Node<'a>, &'a str, bool)> {
        let args = call_node.child_by_field_name("arguments")?;

        // Collect all arguments
        let mut cursor = args.walk();
        let mut arguments = Vec::new();
        for child in args.children(&mut cursor) {
            if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                arguments.push(child);
            }
        }

        // Most g_param_spec_* functions have the pattern:
        // g_param_spec_*(name, nick, blurb, ..., flags)
        // name (0), nick (1), blurb (2) should be string literals
        // flags is typically the last argument

        if arguments.len() < 4 {
            return None;
        }

        // Check if name, nick, blurb are string literals
        let nick = ast_context.get_node_text(arguments[1], source);
        let blurb = ast_context.get_node_text(arguments[2], source);

        // Check if they're string literals (or NULL for nick/blurb which is fine)
        let name_is_literal = arguments[0].kind() == "string_literal";
        let nick_is_literal_or_null =
            arguments[1].kind() == "string_literal" || ast_context.is_null_literal(nick);
        let blurb_is_literal_or_null =
            arguments[2].kind() == "string_literal" || ast_context.is_null_literal(blurb);

        // Only suggest if they're all literals/NULL
        if !name_is_literal || !nick_is_literal_or_null || !blurb_is_literal_or_null {
            return None;
        }

        // Get the flags argument (last argument)
        let flags_arg = *arguments.last()?;
        let flags_text = ast_context.get_node_text(flags_arg, source);

        // Check if flags already contains G_PARAM_STATIC_STRINGS
        let has_static_strings = flags_text.contains("G_PARAM_STATIC_STRINGS")
            || flags_text.contains("G_PARAM_STATIC_NAME")
                && flags_text.contains("G_PARAM_STATIC_NICK")
                && flags_text.contains("G_PARAM_STATIC_BLURB");

        Some((flags_arg, flags_text, has_static_strings))
    }
}
