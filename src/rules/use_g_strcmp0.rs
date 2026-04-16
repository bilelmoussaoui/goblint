use gobject_ast::{Expression, Statement};

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGStrcmp0;

impl Rule for UseGStrcmp0 {
    fn name(&self) -> &'static str {
        "use_g_strcmp0"
    }

    fn description(&self) -> &'static str {
        "Use g_strcmp0 instead of strcmp (NULL-safe)"
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
        func: &gobject_ast::FunctionInfo,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        if !func.is_definition {
            return;
        }

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
            match stmt {
                Statement::If(if_stmt) => {
                    // Check for misuse and proper use in condition
                    self.check_condition(&if_stmt.condition, file_path, violations);
                    self.check_statements(&if_stmt.then_body, file_path, violations);
                    if let Some(else_body) = &if_stmt.else_body {
                        self.check_statements(else_body, file_path, violations);
                    }
                }
                Statement::Return(_) => {
                    // strcmp/g_strcmp0 in return statements is OK (comparison
                    // functions)
                }
                Statement::Compound(compound) => {
                    self.check_statements(&compound.statements, file_path, violations);
                }
                Statement::Labeled(labeled) => {
                    self.check_statements(
                        std::slice::from_ref(&labeled.statement),
                        file_path,
                        violations,
                    );
                }
                _ => {}
            }
        }
    }

    fn check_condition(
        &self,
        condition: &Expression,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        match condition {
            // Bare call: if (strcmp(a, b)) or if (g_strcmp0(a, b))
            Expression::Call(call) if self.is_str_compare(&call.function) => {
                violations.push(self.violation(
                    file_path,
                    call.location.line,
                    call.location.column,
                    format!(
                        "{}() returns 0 for equality — use '{}(...) == 0' or '{}(...) != 0' instead of bare boolean check",
                        call.function, call.function, call.function
                    ),
                ));
            }
            // Negated call: if (!strcmp(a, b)) or if (!g_strcmp0(a, b))
            Expression::Unary(unary) if unary.operator == "!" => {
                if let Expression::Call(call) = &*unary.operand
                    && self.is_str_compare(&call.function)
                {
                    violations.push(self.violation(
                            file_path,
                            call.location.line,
                            call.location.column,
                            format!(
                                "{}() returns 0 for equality — use '{}(...) == 0' or '{}(...) != 0' instead of bare boolean check",
                                call.function, call.function, call.function
                            ),
                        ));
                }
            }
            // Binary expression: check for strcmp in proper comparison
            Expression::Binary(bin) => {
                self.check_strcmp_in_comparison(&bin.left, file_path, violations);
                self.check_strcmp_in_comparison(&bin.right, file_path, violations);
            }
            _ => {}
        }
    }

    fn check_strcmp_in_comparison(
        &self,
        expr: &Expression,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        // Check for strcmp (not g_strcmp0) in a comparison context
        if let Expression::Call(call) = expr {
            if call.function == "strcmp" {
                let message = "Consider g_strcmp0 instead of strcmp if arguments can be NULL (g_strcmp0 is NULL-safe)";

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
                    message.to_string(),
                    fix,
                ));
            } else if call.function == "strncmp" {
                violations.push(self.violation(
                    file_path,
                    call.location.line,
                    call.location.column,
                    "Consider g_strcmp0 or check for NULL first instead of strncmp (if NULL-safety needed)".to_string(),
                ));
            }
        }
    }

    fn is_str_compare(&self, func_name: &str) -> bool {
        func_name == "strcmp" || func_name == "g_strcmp0"
    }
}
