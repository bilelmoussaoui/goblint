use tree_sitter::Node;

use super::{CheckContext, Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGVariantNewTyped;

impl Rule for UseGVariantNewTyped {
    fn name(&self) -> &'static str {
        "use_g_variant_new_typed"
    }

    fn description(&self) -> &'static str {
        "Prefer g_variant_new_string/boolean/etc over g_variant_new with format strings"
    }

    fn category(&self) -> super::Category {
        super::Category::Style
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

impl UseGVariantNewTyped {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        // Look for g_variant_new calls
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
        {
            let func_name = ast_context.get_node_text(function, ctx.source);
            if func_name == "g_variant_new"
                && let Some((format_str, typed_func, rest_args)) =
                    self.extract_variant_new_pattern(ast_context, node, ctx.source)
            {
                // Calculate spacing between function name and opening paren
                if let Some(args_node) = node.child_by_field_name("arguments") {
                    let spacing_start = function.end_byte();
                    let spacing_end = args_node.start_byte();
                    let spacing = ctx.source_text(spacing_start, spacing_end);

                    // Build replacement
                    let replacement = if rest_args.is_empty() {
                        format!("{}{}()", typed_func, spacing)
                    } else {
                        format!("{}{}({})", typed_func, spacing, rest_args)
                    };

                    let fix = Fix::from_range(
                        function.start_byte(),
                        args_node.end_byte(),
                        ctx,
                        &replacement,
                    );

                    violations.push(self.violation_with_fix(
                        ctx.file_path,
                        ctx.base_line + node.start_position().row,
                        node.start_position().column + 1,
                        format!(
                            "Use {} instead of g_variant_new(\"{}\", ...) for type safety",
                            replacement, format_str
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

    /// Extract g_variant_new pattern and return (format_string,
    /// typed_function_name, rest_of_args)
    fn extract_variant_new_pattern<'a>(
        &self,
        ast_context: &AstContext,
        call_node: Node,
        source: &'a [u8],
    ) -> Option<(&'a str, &'static str, String)> {
        let args = call_node.child_by_field_name("arguments")?;

        // Collect all arguments (skip parentheses and commas)
        let mut cursor = args.walk();
        let mut arguments = Vec::new();
        for child in args.children(&mut cursor) {
            if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                arguments.push(child);
            }
        }

        // Need at least 1 argument (the format string)
        if arguments.is_empty() {
            return None;
        }

        let first_arg = arguments[0];

        // Check if first argument is a string literal
        if first_arg.kind() != "string_literal" {
            return None;
        }

        let format_text = ast_context.get_node_text(first_arg, source);
        let format_str = format_text.trim_matches('"');

        // Map format string to typed function
        let typed_func = match format_str {
            "s" => "g_variant_new_string",
            "b" => "g_variant_new_boolean",
            "y" => "g_variant_new_byte",
            "n" => "g_variant_new_int16",
            "q" => "g_variant_new_uint16",
            "i" => "g_variant_new_int32",
            "u" => "g_variant_new_uint32",
            "x" => "g_variant_new_int64",
            "t" => "g_variant_new_uint64",
            "h" => "g_variant_new_handle",
            "d" => "g_variant_new_double",
            "o" => "g_variant_new_object_path",
            "g" => "g_variant_new_signature",
            "v" => "g_variant_new_variant",
            _ => return None, // Not a simple type we can convert
        };

        // Collect remaining arguments (after format string)
        let rest_args = if arguments.len() > 1 {
            let rest: Vec<&str> = arguments[1..]
                .iter()
                .map(|arg| ast_context.get_node_text(*arg, source))
                .collect();
            rest.join(", ")
        } else {
            String::new()
        };

        Some((format_str, typed_func, rest_args))
    }
}
