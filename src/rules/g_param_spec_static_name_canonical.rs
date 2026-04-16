use gobject_ast::{CallExpression, Expression};

use super::{Fix, Rule};
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

    fn check_func_impl(
        &self,
        _ast_context: &AstContext,
        _config: &Config,
        func: &gobject_ast::FunctionInfo,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        if !func.is_definition {
            return;
        }

        // Find all g_param_spec_* calls (but skip g_param_spec_internal)
        for call in func.find_calls_matching(|name| {
            name.starts_with("g_param_spec_") && name != "g_param_spec_internal"
        }) {
            self.check_call(path, call, violations);
        }
    }
}

impl GParamSpecStaticNameCanonical {
    fn check_call(
        &self,
        file_path: &std::path::Path,
        call: &CallExpression,
        violations: &mut Vec<Violation>,
    ) {
        // g_param_spec_* functions have different signatures, but all have:
        // - First argument: name (string)
        // - Last argument: flags (GParamFlags)
        if call.arguments.len() < 2 {
            return;
        }

        // Extract the property name, handling macros like I_("name")
        let Some(name_value) = call.extract_string_from_arg(0) else {
            return;
        };

        if !name_value.contains('_') {
            return; // Name is already canonical
        }

        // Check if flags contain G_PARAM_STATIC_NAME or G_PARAM_STATIC_STRINGS
        let flags_arg = call.arguments.last().unwrap();
        let gobject_ast::Argument::Expression(flags_expr) = flags_arg;

        let has_static_name = flags_expr.contains_identifier("G_PARAM_STATIC_NAME")
            || flags_expr.contains_identifier("G_PARAM_STATIC_STRINGS");

        // Name is non-canonical - create a fix
        let canonical_name = name_value.replace('_', "-");
        let replacement = format!("\"{}\"", canonical_name);

        // Find the actual string literal to replace
        let name_arg = &call.arguments[0];
        let gobject_ast::Argument::Expression(expr) = name_arg;

        let string_lit_location = match expr.as_ref() {
            Expression::StringLiteral(lit) => &lit.location,
            Expression::MacroCall(macro_call) => {
                // Find the string literal inside the macro
                let Some(gobject_ast::Argument::Expression(inner_expr)) =
                    macro_call.arguments.first()
                else {
                    return; // Macro has no arguments
                };

                if let Expression::StringLiteral(lit) = inner_expr.as_ref() {
                    &lit.location
                } else {
                    return; // Macro argument is not a string literal
                }
            }
            _ => return, // Unexpected structure
        };

        let fix = Fix::new(
            string_lit_location.start_byte,
            string_lit_location.end_byte,
            replacement.clone(),
        );

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
            file_path,
            string_lit_location.line,
            string_lit_location.column,
            message,
            fix,
        ));
    }
}
