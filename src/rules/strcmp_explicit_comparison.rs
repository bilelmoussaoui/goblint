use gobject_ast::{Expression, Statement, UnaryOp};

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct StrcmpExplicitComparison;

impl Rule for StrcmpExplicitComparison {
    fn name(&self) -> &'static str {
        "strcmp_explicit_comparison"
    }

    fn description(&self) -> &'static str {
        "Require explicit comparison with 0 for strcmp/g_strcmp0 (returns 0 for equality, not TRUE)"
    }

    fn category(&self) -> super::Category {
        super::Category::Suspicious
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

impl StrcmpExplicitComparison {
    fn check_statements(
        &self,
        statements: &[Statement],
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        for stmt in statements {
            stmt.walk(&mut |s| {
                if let Statement::If(if_stmt) = s {
                    self.check_condition(&if_stmt.condition, file_path, violations);
                }
            });
        }
    }

    fn check_condition(
        &self,
        condition: &Expression,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        match condition {
            // Binary expression: check if it's a comparison with strcmp, or recurse for logical ops
            Expression::Binary(binary) => {
                use gobject_ast::BinaryOp;

                // If it's a comparison operator, don't flag strcmp calls on either side
                // (they already have explicit comparison)
                match binary.operator {
                    BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual => {
                        // Don't recurse - strcmp calls here are OK
                    }
                    // For logical operators, recurse into both sides
                    BinaryOp::LogicalAnd | BinaryOp::LogicalOr => {
                        self.check_condition(&binary.left, file_path, violations);
                        self.check_condition(&binary.right, file_path, violations);
                    }
                    _ => {
                        // For other binary operators, recurse
                        self.check_condition(&binary.left, file_path, violations);
                        self.check_condition(&binary.right, file_path, violations);
                    }
                }
            }
            // Bare call: if (strcmp(a, b)) or if (g_strcmp0(a, b))
            Expression::Call(call) if self.is_str_compare(&call.function) => {
                // Fix: add "!= 0" after the call
                let fix = Fix::new(
                    call.location.end_byte,
                    call.location.end_byte,
                    " != 0".to_string(),
                );

                violations.push(self.violation_with_fix(
                    file_path,
                    call.location.line,
                    call.location.column,
                    format!(
                        "{}() returns 0 for equality — add explicit comparison: '{}(...) != 0'",
                        call.function, call.function
                    ),
                    fix,
                ));
            }
            // Negated call: if (!strcmp(a, b)) or if (!g_strcmp0(a, b))
            Expression::Unary(unary) if unary.operator == UnaryOp::Not => {
                if let Expression::Call(call) = &*unary.operand
                    && self.is_str_compare(&call.function)
                {
                    // Fix: remove the '!' and add ' == 0' after the call
                    let fixes = vec![
                        // Remove the '!' operator
                        Fix::new(
                            unary.location.start_byte,
                            call.location.start_byte,
                            String::new(),
                        ),
                        // Add ' == 0' after the call
                        Fix::new(
                            call.location.end_byte,
                            call.location.end_byte,
                            " == 0".to_string(),
                        ),
                    ];

                    violations.push(self.violation_with_fixes(
                        file_path,
                        call.location.line,
                        call.location.column,
                        format!(
                            "{}() returns 0 for equality — use '{}(...) == 0' instead of '!{}(...)'",
                            call.function, call.function, call.function
                        ),
                        fixes,
                    ));
                }
            }
            _ => {}
        }
    }

    fn is_str_compare(&self, func_name: &str) -> bool {
        matches!(func_name, "strcmp" | "g_strcmp0")
    }
}
