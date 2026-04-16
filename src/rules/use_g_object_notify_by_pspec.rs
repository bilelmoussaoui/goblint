use gobject_ast::Expression;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGObjectNotifyByPspec;

impl Rule for UseGObjectNotifyByPspec {
    fn name(&self) -> &'static str {
        "use_g_object_notify_by_pspec"
    }

    fn description(&self) -> &'static str {
        "Suggest g_object_notify_by_pspec instead of g_object_notify for better performance"
    }

    fn category(&self) -> super::Category {
        super::Category::Perf
    }

    fn fixable(&self) -> bool {
        false
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

        for call in func.find_calls(&["g_object_notify"]) {
            self.check_call(path, call, violations);
        }
    }
}

impl UseGObjectNotifyByPspec {
    fn check_call(
        &self,
        file_path: &std::path::Path,
        call: &gobject_ast::CallExpression,
        violations: &mut Vec<Violation>,
    ) {
        // Need exactly 2 arguments: object and property name
        if call.arguments.len() != 2 {
            return;
        }

        // Check if second argument is a string literal
        let gobject_ast::Argument::Expression(property_expr) = &call.arguments[1];
        if !property_expr.is_string_literal() {
            return;
        }

        // Get the string literal value
        let Expression::StringLiteral(string_lit) = property_expr.as_ref() else {
            unreachable!();
        };

        let property_name = string_lit.value.trim_matches('"');

        // Convert property-name to PROP_NAME for the suggestion
        let property_constant = self.property_name_to_constant(property_name);

        violations.push(self.violation(
            file_path,
            call.location.line,
            call.location.column,
            format!(
                "Use g_object_notify_by_pspec(obj, properties[{}]) instead of g_object_notify(obj, \"{}\") for better performance",
                property_constant, property_name
            ),
        ));
    }

    /// Convert property-name to PROP_NAME constant style
    fn property_name_to_constant(&self, property_name: &str) -> String {
        // Convert kebab-case or camelCase to UPPER_SNAKE_CASE
        let mut result = String::with_capacity(property_name.len() + 5);
        result.push_str("PROP_");

        for c in property_name.chars() {
            if c == '-' {
                result.push('_');
            } else {
                result.push(c.to_ascii_uppercase());
            }
        }

        result
    }
}
