use gobject_ast::Statement;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGAutoptrError;

impl Rule for UseGAutoptrError {
    fn name(&self) -> &'static str {
        "use_g_autoptr_error"
    }

    fn description(&self) -> &'static str {
        "Suggest g_autoptr(GError) instead of manual g_error_free"
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

        self.check_function(func, path, violations);
    }
}

impl UseGAutoptrError {
    fn check_function(
        &self,
        func: &gobject_ast::FunctionInfo,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        // Find all GError* declarations
        let gerror_vars = self.find_gerror_declarations(&func.body_statements);

        // For each GError* variable, check if it's manually freed
        for (var_name, location) in &gerror_vars {
            if self.has_error_free_call(&func.body_statements, var_name) {
                violations.push(self.violation(
                    file_path,
                    location.line,
                    location.column,
                    format!(
                        "Consider using g_autoptr(GError) {} instead of manual g_error_free",
                        var_name
                    ),
                ));
            }
        }
    }

    fn find_gerror_declarations(
        &self,
        statements: &[Statement],
    ) -> Vec<(String, gobject_ast::SourceLocation)> {
        let mut result = Vec::new();
        self.collect_gerror_vars(statements, &mut result);
        result
    }

    fn collect_gerror_vars(
        &self,
        statements: &[Statement],
        result: &mut Vec<(String, gobject_ast::SourceLocation)>,
    ) {
        for stmt in statements {
            match stmt {
                Statement::Declaration(decl) => {
                    // Check if type contains "GError"
                    if decl.type_name.contains("GError") {
                        result.push((decl.name.clone(), decl.location.clone()));
                    }
                }
                Statement::Compound(compound) => {
                    self.collect_gerror_vars(&compound.statements, result);
                }
                Statement::If(if_stmt) => {
                    self.collect_gerror_vars(&if_stmt.then_body, result);
                    if let Some(else_body) = &if_stmt.else_body {
                        self.collect_gerror_vars(else_body, result);
                    }
                }
                Statement::Labeled(labeled) => {
                    self.collect_gerror_vars(std::slice::from_ref(&labeled.statement), result);
                }
                _ => {}
            }
        }
    }

    fn has_error_free_call(&self, statements: &[Statement], var_name: &str) -> bool {
        self.find_error_free_call(statements, var_name)
    }

    fn find_error_free_call(&self, statements: &[Statement], var_name: &str) -> bool {
        use gobject_ast::Expression;

        for stmt in statements {
            match stmt {
                Statement::Expression(expr_stmt) => {
                    if let Expression::Call(call) = &expr_stmt.expr
                        && call.function == "g_error_free"
                        && !call.arguments.is_empty()
                    {
                        // Check if argument matches var_name
                        let gobject_ast::Argument::Expression(arg_expr) = &call.arguments[0];
                        if let Some(arg_var) = arg_expr.extract_variable_name()
                            && arg_var == var_name
                        {
                            return true;
                        }
                    }
                }
                Statement::Compound(compound) => {
                    if self.find_error_free_call(&compound.statements, var_name) {
                        return true;
                    }
                }
                Statement::If(if_stmt) => {
                    if self.find_error_free_call(&if_stmt.then_body, var_name) {
                        return true;
                    }
                    if let Some(else_body) = &if_stmt.else_body
                        && self.find_error_free_call(else_body, var_name)
                    {
                        return true;
                    }
                }
                Statement::Labeled(labeled) => {
                    if self.find_error_free_call(std::slice::from_ref(&labeled.statement), var_name)
                    {
                        return true;
                    }
                }
                _ => {}
            }
        }

        false
    }
}
