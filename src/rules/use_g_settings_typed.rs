use gobject_ast::Expression;

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGSettingsTyped;

impl Rule for UseGSettingsTyped {
    fn name(&self) -> &'static str {
        "use_g_settings_typed"
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

    fn check_func_impl(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        func: &gobject_ast::FunctionInfo,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        if !func.is_definition {
            return;
        }

        let source = &ast_context.project.files.get(path).unwrap().source;

        // Check for g_settings_set_value calls
        for call in func.find_calls(&["g_settings_set_value"]) {
            self.check_settings_set_call(path, call, source, violations);
        }

        // Check for g_variant_get_* calls
        for call in func.find_calls(&[
            "g_variant_get_string",
            "g_variant_get_boolean",
            "g_variant_get_byte",
            "g_variant_get_int16",
            "g_variant_get_uint16",
            "g_variant_get_int32",
            "g_variant_get_uint32",
            "g_variant_get_int64",
            "g_variant_get_uint64",
            "g_variant_get_double",
            "g_variant_get_strv",
        ]) {
            self.check_variant_get_call(path, call, source, violations);
        }
    }
}

impl UseGSettingsTyped {
    fn check_settings_set_call(
        &self,
        file_path: &std::path::Path,
        call: &gobject_ast::CallExpression,
        source: &[u8],
        violations: &mut Vec<Violation>,
    ) {
        // g_settings_set_value(settings, key, variant)
        if call.arguments.len() != 3 {
            return;
        }

        // Check if third argument is g_variant_new call
        let gobject_ast::Argument::Expression(third_expr) = &call.arguments[2];
        let Expression::Call(variant_call) = third_expr.as_ref() else {
            return;
        };

        if variant_call.function != "g_variant_new" {
            return;
        }

        // Extract the pattern from g_variant_new
        let Some((_format_str, typed_func, value_args)) =
            self.extract_variant_pattern(variant_call, source)
        else {
            return;
        };

        let Some(settings_arg) = call.get_arg_text(0, source) else {
            return;
        };
        let Some(key_arg) = call.get_arg_text(1, source) else {
            return;
        };

        // Build replacement
        let replacement = if value_args.is_empty() {
            format!("{} ({}, {})", typed_func, settings_arg, key_arg)
        } else {
            format!(
                "{} ({}, {}, {})",
                typed_func, settings_arg, key_arg, value_args
            )
        };

        let fix = Fix::new(
            call.location.start_byte,
            call.location.end_byte,
            replacement.clone(),
        );

        violations.push(self.violation_with_fix(
            file_path,
            call.location.line,
            call.location.column,
            format!(
                "Use {} instead of g_settings_set_value with g_variant_new for type safety",
                replacement
            ),
            fix,
        ));
    }

    fn check_variant_get_call(
        &self,
        file_path: &std::path::Path,
        call: &gobject_ast::CallExpression,
        source: &[u8],
        violations: &mut Vec<Violation>,
    ) {
        // g_variant_get_*(variant, ...) - first arg should be g_settings_get_value call
        if call.arguments.is_empty() {
            return;
        }

        // Check if first argument is g_settings_get_value call
        let gobject_ast::Argument::Expression(first_expr) = &call.arguments[0];
        let Expression::Call(inner_call) = first_expr.as_ref() else {
            return;
        };

        if inner_call.function != "g_settings_get_value" {
            return;
        }

        // g_settings_get_value(settings, key)
        if inner_call.arguments.len() < 2 {
            return;
        }

        let Some(settings_arg) = inner_call.get_arg_text(0, source) else {
            return;
        };
        let Some(key_arg) = inner_call.get_arg_text(1, source) else {
            return;
        };

        // Map g_variant_get_* to g_settings_get_*
        let typed_func = match call.function.as_str() {
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
            _ => return,
        };

        // Build replacement
        let replacement = format!("{} ({}, {})", typed_func, settings_arg, key_arg);

        let fix = Fix::new(
            call.location.start_byte,
            call.location.end_byte,
            replacement.clone(),
        );

        violations.push(self.violation_with_fix(
            file_path,
            call.location.line,
            call.location.column,
            format!(
                "Use {} instead of g_variant_get_* with g_settings_get_value for type safety",
                replacement
            ),
            fix,
        ));
    }

    /// Extract g_variant_new pattern and return (format_string,
    /// typed_function_name, rest_of_args)
    fn extract_variant_pattern(
        &self,
        variant_call: &gobject_ast::CallExpression,
        source: &[u8],
    ) -> Option<(String, &'static str, String)> {
        // Need at least 1 argument (the format string)
        if variant_call.arguments.is_empty() {
            return None;
        }

        // Check if first argument is a string literal
        let gobject_ast::Argument::Expression(first_expr) = &variant_call.arguments[0];
        let Expression::StringLiteral(string_lit) = first_expr.as_ref() else {
            return None;
        };

        let format_str = string_lit.value.trim_matches('"');

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
        let rest_args = if variant_call.arguments.len() > 1 {
            let rest: Vec<String> = variant_call.arguments[1..]
                .iter()
                .filter_map(|arg| arg.to_source_string(source))
                .collect();
            rest.join(", ")
        } else {
            String::new()
        };

        Some((format_str.to_string(), typed_func, rest_args))
    }
}
