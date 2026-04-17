use gobject_ast::Expression;

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGNew;

impl Rule for UseGNew {
    fn name(&self) -> &'static str {
        "use_g_new"
    }

    fn description(&self) -> &'static str {
        "Suggest g_new/g_new0 instead of g_malloc/g_malloc0 with sizeof for type safety"
    }

    fn category(&self) -> super::Category {
        super::Category::Complexity
    }

    fn fixable(&self) -> bool {
        true
    }

    fn check_func_impl(
        &self,
        _ast_context: &AstContext,
        _config: &Config,
        func: &gobject_ast::top_level::FunctionDefItem,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        for call in func.find_calls(&["g_malloc", "g_malloc0"]) {
            self.check_call(path, call, violations);
        }
    }
}

impl UseGNew {
    fn check_call(
        &self,
        file_path: &std::path::Path,
        call: &gobject_ast::CallExpression,
        violations: &mut Vec<Violation>,
    ) {
        // Need exactly 1 argument
        if call.arguments.len() != 1 {
            return;
        }

        // Check if argument is sizeof(Type)
        let Some(arg_expr) = call.get_arg(0) else {
            return;
        };
        let Expression::Sizeof(sizeof_expr) = arg_expr else {
            return;
        };

        // Extract the type - only works for simple types/identifiers
        let Some(type_name) = sizeof_expr.type_name() else {
            // Complex expression, not a simple type - skip
            return;
        };

        let suggested_func = if call.function == "g_malloc0" {
            "g_new0"
        } else {
            "g_new"
        };

        let replacement = format!("{} ({}, 1)", suggested_func, type_name);

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
                "Use {} instead of {}(sizeof({})) for type safety",
                replacement, call.function, type_name
            ),
            fix,
        ));
    }
}
