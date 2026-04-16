use gobject_ast::Expression;

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGValueSetStaticString;

impl Rule for UseGValueSetStaticString {
    fn name(&self) -> &'static str {
        "use_g_value_set_static_string"
    }

    fn description(&self) -> &'static str {
        "Use g_value_set_static_string for string literals instead of g_value_set_string"
    }

    fn category(&self) -> super::Category {
        super::Category::Perf
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
        for call in func.find_calls(&["g_value_set_string"]) {
            self.check_call(path, call, source, violations);
        }
    }
}

impl UseGValueSetStaticString {
    fn check_call(
        &self,
        file_path: &std::path::Path,
        call: &gobject_ast::CallExpression,
        source: &[u8],
        violations: &mut Vec<Violation>,
    ) {
        // Need at least 2 arguments
        if call.arguments.len() < 2 {
            return;
        }

        // Check if second argument is a string literal
        let gobject_ast::Argument::Expression(second_expr) = &call.arguments[1];
        if !second_expr.is_string_literal() {
            return;
        }

        // Get the string literal for the message
        let Expression::StringLiteral(string_lit) = second_expr.as_ref() else {
            unreachable!();
        };

        // Build the fix - replace just the function name
        let replacement = format!(
            "g_value_set_static_string ({})",
            call.arguments
                .iter()
                .filter_map(|arg| arg.to_source_string(source))
                .collect::<Vec<_>>()
                .join(", ")
        );

        let fix = Fix::new(
            call.location.start_byte,
            call.location.end_byte,
            replacement,
        );

        violations.push(self.violation_with_fix(
            file_path,
            call.location.line,
            call.location.column,
            format!(
                "Use g_value_set_static_string instead of g_value_set_string for string literal {}",
                string_lit.value
            ),
            fix,
        ));
    }
}
