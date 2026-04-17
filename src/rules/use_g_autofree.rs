use std::collections::HashMap;

use gobject_ast::Statement;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGAutofree;

impl Rule for UseGAutofree {
    fn name(&self) -> &'static str {
        "use_g_autofree"
    }

    fn description(&self) -> &'static str {
        "Suggest g_autofree for string/buffer types instead of manual g_free"
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
        // Find all local pointer declarations
        let local_vars = self.find_local_pointer_vars(&func.body_statements);

        // For each variable, check if it's a candidate for g_autofree
        for (var_name, (var_type, location)) in &local_vars {
            // Only suggest g_autofree for simple types (char*, guint8*, void*, etc.)
            // Not for GObject* types (those should use g_autoptr)
            if !var_type.contains('*') {
                continue;
            }

            // Check if variable is allocated
            let is_allocated = self.is_var_allocated(&func.body_statements, var_name);

            // Check if variable is manually freed
            let is_manually_freed = self.is_var_manually_freed(&func.body_statements, var_name);

            // Check if variable is returned
            let is_returned = self.is_var_returned(&func.body_statements, var_name);

            // Suggest g_autofree if:
            // 1. Variable is allocated
            // 2. Variable is manually freed
            // 3. Variable is not returned (would need g_steal_pointer)
            if is_allocated && is_manually_freed && !is_returned {
                violations.push(self.violation(
                    path,
                    location.line,
                    location.column,
                    format!(
                        "Consider using g_autofree {} to avoid manual g_free",
                        var_name
                    ),
                ));
            }
        }
    }
}

impl UseGAutofree {
    fn find_local_pointer_vars(
        &self,
        statements: &[Statement],
    ) -> HashMap<String, (String, gobject_ast::SourceLocation)> {
        let mut result = HashMap::new();
        self.collect_local_vars(statements, &mut result);
        result
    }

    fn collect_local_vars(
        &self,
        statements: &[Statement],
        result: &mut HashMap<String, (String, gobject_ast::SourceLocation)>,
    ) {
        for stmt in statements {
            stmt.walk(&mut |s| {
                if let Statement::Declaration(decl) = s {
                    // Skip if var name contains -> or . (field access)
                    if !decl.name.contains("->") && !decl.name.contains('.') {
                        result.insert(decl.name.clone(), (decl.type_name.clone(), decl.location));
                    }
                }
            });
        }
    }

    fn is_var_allocated(&self, statements: &[Statement], var_name: &str) -> bool {
        use gobject_ast::Expression;

        for stmt in statements {
            let mut found = false;
            stmt.walk(&mut |s| {
                match s {
                    // Check init: char *var = g_strdup(...)
                    Statement::Declaration(decl) => {
                        if decl.name == var_name
                            && let Some(Expression::Call(call)) = &decl.initializer
                            && self.is_autofree_allocation(&call.function)
                        {
                            found = true;
                        }
                    }
                    // Check assignment: var = g_strdup(...)
                    Statement::Expression(expr_stmt) => {
                        if let Expression::Assignment(assign) = &expr_stmt.expr
                            && assign.lhs == var_name
                            && let Expression::Call(call) = &*assign.rhs
                            && self.is_autofree_allocation(&call.function)
                        {
                            found = true;
                        }
                    }
                    _ => {}
                }
            });
            if found {
                return true;
            }
        }
        false
    }

    fn is_autofree_allocation(&self, func_name: &str) -> bool {
        // Functions that allocate memory suitable for g_autofree
        matches!(
            func_name,
            "g_strdup"
                | "g_strndup"
                | "g_strdup_printf"
                | "g_strdup_vprintf"
                | "g_malloc"
                | "g_malloc0"
                | "g_realloc"
                | "g_try_malloc"
                | "g_try_malloc0"
                | "g_memdup"
                | "g_new"
                | "g_new0"
        )
    }

    fn is_var_manually_freed(&self, statements: &[Statement], var_name: &str) -> bool {
        use gobject_ast::Expression;

        for stmt in statements {
            let mut found = false;
            stmt.walk(&mut |s| {
                if let Statement::Expression(expr_stmt) = s
                    && let Expression::Call(call) = &expr_stmt.expr
                    && call.function == "g_free"
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

    fn is_var_returned(&self, statements: &[Statement], var_name: &str) -> bool {
        use gobject_ast::Expression;

        for stmt in statements {
            let mut found = false;
            stmt.walk(&mut |s| {
                if let Statement::Return(ret) = s
                    && let Some(Expression::Identifier(id)) = &ret.value
                    && id.name == var_name
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
