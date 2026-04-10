use tree_sitter::Node;

use super::{CheckContext, Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct PreferGSettingsTyped;

impl Rule for PreferGSettingsTyped {
    fn name(&self) -> &'static str {
        "prefer_g_settings_typed"
    }

    fn description(&self) -> &'static str {
        "Prefer g_settings_get/set_string/boolean/etc over g_settings_get/set_value with g_variant"
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

impl PreferGSettingsTyped {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        // Look for g_settings_set_value calls
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
        {
            let func_name = ast_context.get_node_text(function, ctx.source);
            if func_name == "g_settings_set_value" {
                // Check if third argument is a g_variant_new call
                if let Some((settings_arg, key_arg, typed_func, value_args)) =
                    self.extract_settings_set_pattern(ast_context, node, ctx.source)
                {
                    // Calculate spacing
                    if let Some(args_node) = node.child_by_field_name("arguments") {
                        let spacing_start = function.end_byte();
                        let spacing_end = args_node.start_byte();
                        let spacing = std::str::from_utf8(&ctx.source[spacing_start..spacing_end])
                            .unwrap_or("");

                        // Build replacement
                        let replacement = if value_args.is_empty() {
                            format!("{}{}({}, {})", typed_func, spacing, settings_arg, key_arg)
                        } else {
                            format!(
                                "{}{}({}, {}, {})",
                                typed_func, spacing, settings_arg, key_arg, value_args
                            )
                        };

                        let fix = Fix {
                            start_byte: ctx.base_byte + function.start_byte(),
                            end_byte: ctx.base_byte + args_node.end_byte(),
                            replacement: replacement.clone(),
                        };

                        violations.push(self.violation_with_fix(
                            ctx.file_path,
                            ctx.base_line + node.start_position().row,
                            node.start_position().column + 1,
                            format!(
                                "Use {} instead of g_settings_set_value with g_variant_new for type safety",
                                replacement
                            ),
                            fix,
                        ));
                    }
                }
            }
            // Look for g_variant_get_* calls with g_settings_get_value
            else if func_name.starts_with("g_variant_get_")
                && let Some((settings_arg, key_arg, typed_func)) =
                    self.extract_settings_get_pattern(ast_context, node, ctx.source, &func_name)
            {
                // Calculate spacing
                if let Some(args_node) = node.child_by_field_name("arguments") {
                    let spacing_start = function.end_byte();
                    let spacing_end = args_node.start_byte();
                    let spacing =
                        std::str::from_utf8(&ctx.source[spacing_start..spacing_end]).unwrap_or("");

                    // Build replacement
                    let replacement =
                        format!("{}{}({}, {})", typed_func, spacing, settings_arg, key_arg);

                    let fix = Fix {
                        start_byte: ctx.base_byte + function.start_byte(),
                        end_byte: ctx.base_byte + args_node.end_byte(),
                        replacement: replacement.clone(),
                    };

                    violations.push(self.violation_with_fix(
                            ctx.file_path,
                            ctx.base_line + node.start_position().row,
                            node.start_position().column + 1,
                            format!(
                                "Use {} instead of g_variant_get_* with g_settings_get_value for type safety",
                                replacement
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

    /// Extract g_settings_set_value pattern and return (settings, key,
    /// typed_function_name, value_args)
    fn extract_settings_set_pattern(
        &self,
        ast_context: &AstContext,
        call_node: Node,
        source: &[u8],
    ) -> Option<(String, String, &'static str, String)> {
        let args = call_node.child_by_field_name("arguments")?;

        // Collect all arguments
        let mut cursor = args.walk();
        let mut arguments = Vec::new();
        for child in args.children(&mut cursor) {
            if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                arguments.push(child);
            }
        }

        // g_settings_set_value(settings, key, variant)
        if arguments.len() != 3 {
            return None;
        }

        let settings_arg = ast_context.get_node_text(arguments[0], source);
        let key_arg = ast_context.get_node_text(arguments[1], source);
        let variant_arg = arguments[2];

        // Check if third argument is g_variant_new call
        if variant_arg.kind() == "call_expression"
            && let Some(variant_func) = variant_arg.child_by_field_name("function")
        {
            let variant_func_name = ast_context.get_node_text(variant_func, source);
            if variant_func_name == "g_variant_new" {
                // Extract the pattern from g_variant_new
                if let Some((_format_str, typed_func, rest_args)) =
                    self.extract_variant_pattern(ast_context, variant_arg, source)
                {
                    return Some((settings_arg, key_arg, typed_func, rest_args));
                }
            }
        }

        None
    }

    /// Extract g_variant_new pattern and return (format_string,
    /// typed_function_name, rest_of_args)
    fn extract_variant_pattern(
        &self,
        ast_context: &AstContext,
        call_node: Node,
        source: &[u8],
    ) -> Option<(String, &'static str, String)> {
        let args = call_node.child_by_field_name("arguments")?;

        // Collect all arguments
        let mut cursor = args.walk();
        let mut arguments = Vec::new();
        for child in args.children(&mut cursor) {
            if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                arguments.push(child);
            }
        }

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

        // Map format string to typed settings function
        let typed_func = match format_str {
            "s" => "g_settings_set_string",
            "b" => "g_settings_set_boolean",
            "y" => "g_settings_set_byte",
            "n" => "g_settings_set_int",  // int16 → int (closest match)
            "q" => "g_settings_set_uint", // uint16 → uint (closest match)
            "i" => "g_settings_set_int",
            "u" => "g_settings_set_uint",
            "x" => "g_settings_set_int64",
            "t" => "g_settings_set_uint64",
            "d" => "g_settings_set_double",
            "as" => "g_settings_set_strv",
            _ => return None, // Not a simple type we can convert
        };

        // Collect remaining arguments (after format string)
        let rest_args = if arguments.len() > 1 {
            let rest: Vec<String> = arguments[1..]
                .iter()
                .map(|arg| ast_context.get_node_text(*arg, source))
                .collect();
            rest.join(", ")
        } else {
            String::new()
        };

        Some((format_str.to_string(), typed_func, rest_args))
    }

    /// Extract g_variant_get_* pattern with g_settings_get_value and return
    /// (settings, key, typed_function_name)
    fn extract_settings_get_pattern(
        &self,
        ast_context: &AstContext,
        call_node: Node,
        source: &[u8],
        variant_get_func: &str,
    ) -> Option<(String, String, &'static str)> {
        let args = call_node.child_by_field_name("arguments")?;

        // Collect all arguments
        let mut cursor = args.walk();
        let mut arguments = Vec::new();
        for child in args.children(&mut cursor) {
            if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                arguments.push(child);
            }
        }

        // g_variant_get_*(variant, ...) - first arg should be g_settings_get_value call
        if arguments.is_empty() {
            return None;
        }

        let first_arg = arguments[0];

        // Check if first argument is g_settings_get_value call
        if first_arg.kind() == "call_expression"
            && let Some(inner_func) = first_arg.child_by_field_name("function")
        {
            let inner_func_name = ast_context.get_node_text(inner_func, source);
            if inner_func_name == "g_settings_get_value" {
                // Extract settings and key from g_settings_get_value
                if let Some(inner_args) = first_arg.child_by_field_name("arguments") {
                    let mut inner_cursor = inner_args.walk();
                    let mut inner_arguments = Vec::new();
                    for child in inner_args.children(&mut inner_cursor) {
                        if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                            inner_arguments.push(child);
                        }
                    }

                    // g_settings_get_value(settings, key)
                    if inner_arguments.len() >= 2 {
                        let settings_arg = ast_context.get_node_text(inner_arguments[0], source);
                        let key_arg = ast_context.get_node_text(inner_arguments[1], source);

                        // Map g_variant_get_* to g_settings_get_*
                        let typed_func = match variant_get_func {
                            "g_variant_get_string" => "g_settings_get_string",
                            "g_variant_get_boolean" => "g_settings_get_boolean",
                            "g_variant_get_byte" => "g_settings_get_byte",
                            "g_variant_get_int16" => "g_settings_get_int",
                            "g_variant_get_uint16" => "g_settings_get_uint",
                            "g_variant_get_int32" => "g_settings_get_int",
                            "g_variant_get_uint32" => "g_settings_get_uint",
                            "g_variant_get_int64" => "g_settings_get_int64",
                            "g_variant_get_uint64" => "g_settings_get_uint64",
                            "g_variant_get_double" => "g_settings_get_double",
                            "g_variant_get_strv" => "g_settings_get_strv",
                            _ => return None,
                        };

                        return Some((settings_arg, key_arg, typed_func));
                    }
                }
            }
        }

        None
    }
}
