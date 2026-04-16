use gobject_ast::Expression;

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGStringFreeAndSteal;

impl Rule for UseGStringFreeAndSteal {
    fn name(&self) -> &'static str {
        "use_g_string_free_and_steal"
    }

    fn description(&self) -> &'static str {
        "Suggest g_string_free_and_steal instead of g_string_free (..., FALSE) for better readability"
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
        for call in func.find_calls(&["g_string_free"]) {
            self.check_call(path, call, source, violations);
        }
    }
}

impl UseGStringFreeAndSteal {
    fn check_call(
        &self,
        file_path: &std::path::Path,
        call: &gobject_ast::CallExpression,
        source: &[u8],
        violations: &mut Vec<Violation>,
    ) {
        if call.arguments.len() != 2 {
            return;
        }

        // Check if second argument is FALSE/false/0
        let gobject_ast::Argument::Expression(second_expr) = &call.arguments[1];
        let is_false = match second_expr.as_ref() {
            Expression::Boolean(b) => !b.value,
            Expression::NumberLiteral(n) => n.value == "0",
            _ => false,
        };

        if !is_false {
            return;
        }

        // Get argument text for the fix
        let Some(first_text) = call.get_arg_text(0, source) else {
            return;
        };
        let Some(second_text) = call.get_arg_text(1, source) else {
            return;
        };

        // Build replacement
        let replacement = format!("g_string_free_and_steal ({})", first_text);

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
                "Use {} instead of g_string_free({}, {}) for readability",
                replacement, first_text, second_text
            ),
            fix,
        ));
    }
}
