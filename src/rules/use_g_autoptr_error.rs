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
        func: &gobject_ast::top_level::FunctionDefItem,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        self.check_function(func, path, violations);
    }
}

impl UseGAutoptrError {
    fn check_function(
        &self,
        func: &gobject_ast::top_level::FunctionDefItem,
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
            stmt.walk(&mut |s| {
                if let Statement::Declaration(decl) = s {
                    // Check if type contains "GError"
                    if decl.type_name.contains("GError") {
                        result.push((decl.name.clone(), decl.location));
                    }
                }
            });
        }
    }

    fn has_error_free_call(&self, statements: &[Statement], var_name: &str) -> bool {
        use gobject_ast::Expression;

        for stmt in statements {
            let mut found = false;
            stmt.walk(&mut |s| {
                if let Statement::Expression(expr_stmt) = s
                    && let Expression::Call(call) = &expr_stmt.expr
                    && call.function == "g_error_free"
                    && let Some(arg_expr) = call.get_arg(0)
                    && let Some(arg_var) = arg_expr.extract_variable_name()
                    && arg_var == var_name
                {
                    found = true;
                }
            });
            if found {
                return true;
            }
        }
        false
    }
}
