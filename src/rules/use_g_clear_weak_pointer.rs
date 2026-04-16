use gobject_ast::{Expression, Statement};

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGClearWeakPointer;

impl Rule for UseGClearWeakPointer {
    fn name(&self) -> &'static str {
        "use_g_clear_weak_pointer"
    }

    fn description(&self) -> &'static str {
        "Suggest g_clear_weak_pointer instead of manual g_object_remove_weak_pointer and NULL assignment"
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
        func: &gobject_ast::FunctionInfo,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        if !func.is_definition {
            return;
        }

        // Walk through function body looking for the pattern
        self.check_statements(&func.body_statements, path, violations);
    }
}

impl UseGClearWeakPointer {
    fn check_statements(
        &self,
        statements: &[Statement],
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        let mut i = 0;
        while i < statements.len() {
            // Look for g_object_remove_weak_pointer followed by NULL assignment
            if i + 1 < statements.len()
                && self.try_remove_weak_then_null(
                    &statements[i],
                    &statements[i + 1],
                    file_path,
                    violations,
                )
            {
                i += 2;
                continue;
            }

            // Recursively check nested statements
            match &statements[i] {
                Statement::If(if_stmt) => {
                    self.check_statements(&if_stmt.then_body, file_path, violations);
                    if let Some(else_body) = &if_stmt.else_body {
                        self.check_statements(else_body, file_path, violations);
                    }
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

            i += 1;
        }
    }

    /// Matches `g_object_remove_weak_pointer(obj, &var); var = NULL;`
    fn try_remove_weak_then_null(
        &self,
        s1: &Statement,
        s2: &Statement,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) -> bool {
        // First statement must be g_object_remove_weak_pointer call
        let Statement::Expression(expr_stmt) = s1 else {
            return false;
        };

        let Expression::Call(call) = &expr_stmt.expr else {
            return false;
        };

        if call.function != "g_object_remove_weak_pointer" {
            return false;
        }

        // Need at least 2 arguments
        if call.arguments.len() < 2 {
            return false;
        }

        // Extract the variable from the second argument
        let Some(var_name) = self.extract_weak_pointer_var(&call.arguments[1]) else {
            return false;
        };

        // Second statement must be var = NULL
        if !self.is_null_assignment(s2, &var_name) {
            return false;
        }

        // Create a fix
        let replacement = format!("g_clear_weak_pointer (&{});", var_name);
        let fix = Fix::new(
            s1.location().start_byte,
            s2.location().end_byte,
            replacement.clone(),
        );

        violations.push(self.violation_with_fix(
            file_path,
            s1.location().line,
            s1.location().column,
            format!(
                "Use {} instead of g_object_remove_weak_pointer + NULL assignment",
                replacement.trim_end_matches(';')
            ),
            fix,
        ));
        true
    }

    /// Extract variable name from the second argument of
    /// g_object_remove_weak_pointer Pattern: (gpointer*)&var or &var
    fn extract_weak_pointer_var(&self, arg: &gobject_ast::Argument) -> Option<String> {
        let gobject_ast::Argument::Expression(expr) = arg;

        // Handle cast expressions: (gpointer*)&var
        let inner_expr = if let Expression::Cast(cast) = expr.as_ref() {
            &cast.operand
        } else {
            expr
        };

        // Handle unary & operator: &var
        if let Expression::Unary(unary) = inner_expr.as_ref()
            && unary.operator == "&"
        {
            return unary.operand.extract_variable_name();
        }

        None
    }

    /// Check if statement is `var = NULL`
    fn is_null_assignment(&self, stmt: &Statement, var_name: &str) -> bool {
        let Statement::Expression(expr_stmt) = stmt else {
            return false;
        };

        let Expression::Assignment(assign) = &expr_stmt.expr else {
            return false;
        };

        // Check left side matches var_name and right side is NULL
        assign.lhs == var_name && assign.operator == "=" && assign.rhs.is_null()
    }
}
