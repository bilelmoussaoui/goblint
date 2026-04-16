use gobject_ast::{Expression, Statement};

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGObjectNewWithProperties;

impl Rule for UseGObjectNewWithProperties {
    fn name(&self) -> &'static str {
        "use_g_object_new_with_properties"
    }

    fn description(&self) -> &'static str {
        "Suggest setting properties in g_object_new instead of separate g_object_set calls"
    }

    fn category(&self) -> super::Category {
        super::Category::Complexity
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

        // Find all g_object_new calls with no properties
        let empty_new_calls: Vec<_> = func
            .find_calls(&["g_object_new"])
            .into_iter()
            .filter(|call| self.is_g_object_new_empty(call))
            .collect();

        if empty_new_calls.is_empty() {
            return;
        }

        // Check statements for the pattern
        self.check_statements(&func.body_statements, &empty_new_calls, path, violations);
    }
}

impl UseGObjectNewWithProperties {
    fn check_statements(
        &self,
        statements: &[Statement],
        empty_new_calls: &[&gobject_ast::CallExpression],
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        for i in 0..statements.len() {
            // Check if this statement contains one of our empty g_object_new calls
            if let Some((var_name, location)) =
                self.find_empty_new_in_statement(&statements[i], empty_new_calls)
            {
                // Count consecutive g_object_set calls on the same variable
                let mut set_count = 0;

                for next_stmt in statements.iter().skip(i + 1) {
                    if let Some(set_var) = self.extract_g_object_set(next_stmt)
                        && set_var == var_name
                    {
                        set_count += 1;
                        continue;
                    }

                    // Stop if we hit something that's not a g_object_set on our variable
                    break;
                }

                // Only report if there's at least one g_object_set call
                if set_count > 0 {
                    violations.push(self.violation(
                        file_path,
                        location.line,
                        location.column,
                        format!(
                            "Set properties in g_object_new() instead of {} separate g_object_set() call{}",
                            set_count,
                            if set_count > 1 { "s" } else { "" }
                        ),
                    ));
                }
            }

            // Recurse into nested statements
            match &statements[i] {
                Statement::If(if_stmt) => {
                    self.check_statements(
                        &if_stmt.then_body,
                        empty_new_calls,
                        file_path,
                        violations,
                    );
                    if let Some(else_body) = &if_stmt.else_body {
                        self.check_statements(else_body, empty_new_calls, file_path, violations);
                    }
                }
                Statement::Compound(compound) => {
                    self.check_statements(
                        &compound.statements,
                        empty_new_calls,
                        file_path,
                        violations,
                    );
                }
                Statement::Labeled(labeled) => {
                    self.check_statements(
                        std::slice::from_ref(&labeled.statement),
                        empty_new_calls,
                        file_path,
                        violations,
                    );
                }
                _ => {}
            }
        }
    }

    /// Check if a statement contains one of the empty g_object_new calls
    /// Returns (variable_name, statement_location) if found
    fn find_empty_new_in_statement(
        &self,
        stmt: &Statement,
        empty_new_calls: &[&gobject_ast::CallExpression],
    ) -> Option<(String, gobject_ast::SourceLocation)> {
        match stmt {
            // Declaration: FooObject *obj = g_object_new(TYPE, NULL);
            Statement::Declaration(decl) => {
                if let Some(Expression::Call(call)) = &decl.initializer {
                    // Check if this call is one of our empty g_object_new calls
                    for &empty_call in empty_new_calls {
                        if std::ptr::eq(call as *const _, empty_call as *const _) {
                            return Some((decl.name.clone(), decl.location.clone()));
                        }
                    }
                }
            }
            // Assignment: obj = g_object_new(TYPE, NULL);
            Statement::Expression(expr_stmt) => {
                if let Expression::Assignment(assign) = &expr_stmt.expr
                    && let Expression::Call(call) = &*assign.rhs
                {
                    for &empty_call in empty_new_calls {
                        if std::ptr::eq(call as *const _, empty_call as *const _) {
                            return Some((assign.lhs.clone(), expr_stmt.location.clone()));
                        }
                    }
                }
            }
            _ => {}
        }

        None
    }

    /// Check if a call is g_object_new with no properties (just NULL or type
    /// only)
    fn is_g_object_new_empty(&self, call: &gobject_ast::CallExpression) -> bool {
        if call.function != "g_object_new" {
            return false;
        }

        // g_object_new with just type and NULL, or just type
        // g_object_new(TYPE, NULL) - 2 args
        // g_object_new(TYPE) - 1 arg (rare but valid)
        match call.arguments.len() {
            1 => true,
            2 => call.arguments[1].is_null(),
            _ => false,
        }
    }

    /// Extract g_object_set call, return the object variable
    fn extract_g_object_set(&self, stmt: &Statement) -> Option<String> {
        let Statement::Expression(expr_stmt) = stmt else {
            return None;
        };

        let Expression::Call(call) = &expr_stmt.expr else {
            return None;
        };

        if call.function != "g_object_set" {
            return None;
        }

        // Get the first argument (the object)
        if call.arguments.is_empty() {
            return None;
        }

        let gobject_ast::Argument::Expression(expr) = &call.arguments[0];
        expr.extract_variable_name()
    }
}
