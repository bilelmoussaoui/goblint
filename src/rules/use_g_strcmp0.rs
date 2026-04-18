use gobject_ast::{Expression, Statement};

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGStrcmp0;

impl Rule for UseGStrcmp0 {
    fn name(&self) -> &'static str {
        "use_g_strcmp0"
    }

    fn description(&self) -> &'static str {
        "Suggest g_strcmp0 instead of strcmp if arguments can be NULL (NULL-safe)"
    }

    fn category(&self) -> super::Category {
        super::Category::Style
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
        self.check_statements(&func.body_statements, path, violations);
    }
}

impl UseGStrcmp0 {
    fn check_statements(
        &self,
        statements: &[Statement],
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        for stmt in statements {
            stmt.walk(&mut |s| {
                // Walk all expressions in the statement
                s.walk_expressions(&mut |expr| {
                    self.check_expression(expr, file_path, violations);
                });
            });
        }
    }

    fn check_expression(
        &self,
        expr: &Expression,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        // Check for strcmp usage (suggest g_strcmp0 for NULL-safety)
        if let Expression::Call(call) = expr
            && call.function == "strcmp"
        {
            // Create fix to replace "strcmp" with "g_strcmp0"
            let fix = Fix::new(
                call.location.start_byte,
                call.location.start_byte + "strcmp".len(),
                "g_strcmp0".to_string(),
            );

            violations.push(self.violation_with_fix(
                    file_path,
                    call.location.line,
                    call.location.column,
                    "Consider g_strcmp0 instead of strcmp if arguments can be NULL (g_strcmp0 is NULL-safe)".to_string(),
                    fix,
                ));
        }

        // Recursively check nested expressions
        expr.walk(&mut |e| {
            if !std::ptr::eq(e, expr) {
                self.check_expression(e, file_path, violations);
            }
        });
    }
}
