use tree_sitter::Node;

use super::{CheckContext, Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct GParamSpecStaticNameCanonical;

impl Rule for GParamSpecStaticNameCanonical {
    fn name(&self) -> &'static str {
        "g_param_spec_static_name_canonical"
    }

    fn description(&self) -> &'static str {
        "Ensure property names are canonical (use dashes, not underscores)"
    }

    fn category(&self) -> super::Category {
        super::Category::Correctness
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

impl GParamSpecStaticNameCanonical {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        // Look for g_param_spec_* calls (but skip g_param_spec_internal)
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
        {
            let func_name = ast_context.get_node_text(function, ctx.source);

            if func_name.starts_with("g_param_spec_")
                && func_name != "g_param_spec_internal"
                && let Some((name_arg, _flags_arg, has_static_name)) =
                    self.check_param_spec_call(ast_context, node, ctx.source)
            {
                // Extract the actual string, handling macros like I_("name") or N_("name")
                let (name_value, actual_node) =
                    self.extract_string_from_arg(ast_context, name_arg, ctx.source);

                if name_value.contains('_') {
                    // Name is non-canonical - create a fix
                    let canonical_name = name_value.replace('_', "-");
                    let replacement = format!("\"{}\"", canonical_name);

                    let fix = Fix::from_node(actual_node, ctx, replacement);

                    let message = if has_static_name {
                        format!(
                            "Property name '{}' is not canonical (contains underscores). \
                                 With G_PARAM_STATIC_NAME this will cause: \
                                 g_param_spec_internal: assertion '!(flags & G_PARAM_STATIC_NAME) || is_canonical (name)' failed. \
                                 Use '{}' instead",
                            name_value, canonical_name
                        )
                    } else {
                        format!(
                            "Property name '{}' should use dashes instead of underscores. \
                                 Use '{}' for consistency with GObject conventions",
                            name_value, canonical_name
                        )
                    };

                    violations.push(self.violation_with_fix(
                        ctx.file_path,
                        ctx.base_line + actual_node.start_position().row,
                        actual_node.start_position().column + 1,
                        message,
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

    /// Extract string value from an argument, handling macros like I_("name")
    /// Returns (string_value_without_quotes, actual_string_literal_node)
    fn extract_string_from_arg<'a>(
        &self,
        ast_context: &AstContext,
        arg_node: Node<'a>,
        source: &'a [u8],
    ) -> (&'a str, Node<'a>) {
        // Check if it's a macro call like I_("name") or N_("name")
        if arg_node.kind() == "call_expression" {
            // Get the arguments of the macro
            if let Some(arguments) = arg_node.child_by_field_name("arguments") {
                let mut cursor = arguments.walk();
                for child in arguments.children(&mut cursor) {
                    // Find the first string_literal
                    if child.kind() == "string_literal" {
                        let text = ast_context.get_node_text(child, source);
                        let value = text.trim_matches('"');
                        return (value, child);
                    }
                }
            }
        }

        // Otherwise, it should be a direct string literal
        let name_text = ast_context.get_node_text(arg_node, source);
        let name_value = name_text.trim_matches('"');
        (name_value, arg_node)
    }

    /// Check a g_param_spec_* call and return (name_arg, flags_arg,
    /// has_static_name)
    fn check_param_spec_call<'a>(
        &self,
        ast_context: &AstContext,
        call_node: Node<'a>,
        source: &[u8],
    ) -> Option<(Node<'a>, Node<'a>, bool)> {
        let args = call_node.child_by_field_name("arguments")?;

        // Collect arguments
        let mut cursor = args.walk();
        let arguments: Vec<Node> = args
            .children(&mut cursor)
            .filter(|child| child.kind() != "(" && child.kind() != ")" && child.kind() != ",")
            .collect();

        // g_param_spec_* functions have different signatures, but all have:
        // - First argument: name (string)
        // - Last argument: flags (GParamFlags)
        if arguments.len() < 2 {
            return None;
        }

        let name_arg = arguments[0];
        let flags_arg = *arguments.last()?;

        // Check if flags contain G_PARAM_STATIC_NAME or G_PARAM_STATIC_STRINGS
        let flags_text = ast_context.get_node_text(flags_arg, source);
        let has_static_name = flags_text.contains("G_PARAM_STATIC_NAME")
            || flags_text.contains("G_PARAM_STATIC_STRINGS");

        Some((name_arg, flags_arg, has_static_name))
    }
}
