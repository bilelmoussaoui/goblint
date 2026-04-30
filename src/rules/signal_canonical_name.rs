use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct SignalCanonicalName;

impl Rule for SignalCanonicalName {
    fn name(&self) -> &'static str {
        "signal_canonical_name"
    }

    fn description(&self) -> &'static str {
        "Signal names should use hyphens (-) instead of underscores (_)"
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
        for stmt in &func.body_statements {
            stmt.walk(&mut |s| {
                match s {
                    gobject_ast::Statement::Expression(expr_stmt) => {
                        self.check_expression(&expr_stmt.expr, path, violations);
                    }
                    gobject_ast::Statement::Declaration(decl) => {
                        // Check declaration initializer
                        if let Some(ref init_expr) = decl.initializer {
                            self.check_expression(init_expr, path, violations);
                        }
                    }
                    _ => {}
                }
            });
        }
    }
}

impl SignalCanonicalName {
    /// Check an expression for signal function calls
    fn check_expression(
        &self,
        expr: &gobject_ast::Expression,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        use gobject_ast::{Argument, Expression};

        match expr {
            Expression::Call(call) => {
                // Check if this is a signal-related function
                let func_name = call.function_name();
                match func_name.as_str() {
                    // Signal creation/lookup functions - signal name is first argument
                    "g_signal_new"
                    | "g_signal_newv"
                    | "g_signal_new_valist"
                    | "g_signal_new_class_handler"
                    | "g_signal_lookup" => {
                        if let Some(Argument::Expression(arg_expr)) = call.arguments.first() {
                            self.check_signal_name_arg(arg_expr, path, violations);
                        }
                    }
                    // Signal connection functions - signal name is second argument
                    "g_signal_connect"
                    | "g_signal_connect_after"
                    | "g_signal_connect_swapped"
                    | "g_signal_connect_data"
                    | "g_signal_connect_object"
                    | "g_signal_emit_by_name"
                    | "g_signal_group_connect"
                    | "g_signal_group_connect_after"
                    | "g_signal_group_connect_swapped"
                    | "g_signal_group_connect_object" => {
                        if let Some(Argument::Expression(arg_expr)) = call.arguments.get(1) {
                            self.check_signal_name_arg(arg_expr, path, violations);
                        }
                    }
                    _ => {}
                }

                // Recursively check nested expressions
                for arg in &call.arguments {
                    let Argument::Expression(e) = arg;
                    self.check_expression(e, path, violations);
                }
            }
            Expression::Binary(binary) => {
                self.check_expression(&binary.left, path, violations);
                self.check_expression(&binary.right, path, violations);
            }
            Expression::Unary(unary) => {
                self.check_expression(&unary.operand, path, violations);
            }
            Expression::Assignment(assignment) => {
                self.check_expression(&assignment.lhs, path, violations);
                self.check_expression(&assignment.rhs, path, violations);
            }
            Expression::Cast(cast) => {
                self.check_expression(&cast.operand, path, violations);
            }
            Expression::Conditional(cond) => {
                self.check_expression(&cond.condition, path, violations);
                self.check_expression(&cond.then_expr, path, violations);
                self.check_expression(&cond.else_expr, path, violations);
            }
            _ => {}
        }
    }

    /// Check a signal name argument (should be a string literal)
    fn check_signal_name_arg(
        &self,
        expr: &gobject_ast::Expression,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        use gobject_ast::Expression;

        if let Expression::StringLiteral(string_lit) = expr {
            // Remove quotes and check for underscores
            let signal_name = string_lit.value.trim_matches('"');

            if signal_name.contains('_') {
                // Generate the fixed signal name (replace _ with -)
                let fixed_name = signal_name.replace('_', "-");
                let replacement = format!("\"{}\"", fixed_name);

                let fix = Fix::new(
                    string_lit.location.start_byte,
                    string_lit.location.end_byte,
                    replacement,
                );

                violations.push(self.violation_with_fix(
                    path,
                    string_lit.location.line,
                    string_lit.location.column,
                    format!(
                        "Signal name '{}' should use hyphens instead of underscores: '{}'",
                        signal_name, fixed_name
                    ),
                    fix,
                ));
            }
        }
    }
}
