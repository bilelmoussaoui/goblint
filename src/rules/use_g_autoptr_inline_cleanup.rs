use std::collections::HashMap;

use gobject_ast::Statement;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGAutoptrInlineCleanup;

impl Rule for UseGAutoptrInlineCleanup {
    fn name(&self) -> &'static str {
        "use_g_autoptr_inline_cleanup"
    }

    fn description(&self) -> &'static str {
        "Suggest g_autoptr instead of inline manual cleanup (g_object_unref/g_free)"
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

impl UseGAutoptrInlineCleanup {
    fn check_function(
        &self,
        func: &gobject_ast::FunctionInfo,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        // Find all local pointer declarations
        let local_vars = self.find_local_pointer_vars(&func.body_statements);

        // For each variable, check if it's a candidate for g_autoptr
        for (var_name, (var_type, location)) in &local_vars {
            // Check if variable is allocated
            let is_allocated = self.is_var_allocated(&func.body_statements, var_name);

            // Check if variable is manually freed
            let is_manually_freed = self.is_var_manually_freed(&func.body_statements, var_name);

            // Check if variable is returned without being freed
            let is_returned = self.is_var_returned(&func.body_statements, var_name);

            // Suggest g_autoptr if:
            // 1. Variable is allocated
            // 2. Variable is manually freed at least once
            // 3. Variable is not returned directly (would need g_steal_pointer)
            if is_allocated && is_manually_freed && !is_returned {
                let base_type = self.extract_base_type(var_type);
                violations.push(self.violation(
                    file_path,
                    location.line,
                    location.column,
                    format!(
                        "Consider using g_autoptr({}) {} to avoid manual cleanup",
                        base_type, var_name
                    ),
                ));
            }
        }
    }

    fn extract_base_type(&self, type_name: &str) -> String {
        // Extract base type from "const Foo *" -> "Foo"
        type_name
            .trim()
            .trim_start_matches("const")
            .trim()
            .trim_end_matches('*')
            .trim()
            .to_string()
    }

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
                    // Skip variables already using g_autoptr/g_autofree
                    if decl.type_name.contains("g_autoptr") || decl.type_name.contains("g_autofree")
                    {
                        return;
                    }

                    // Only track pointer types for GObject types
                    if self.is_autoptr_candidate(&decl.type_name) {
                        // Skip field access names
                        if !decl.name.contains("->") && !decl.name.contains('.') {
                            result.insert(
                                decl.name.clone(),
                                (decl.type_name.clone(), decl.location.clone()),
                            );
                        }
                    }
                }
            });
        }
    }

    fn is_autoptr_candidate(&self, type_name: &str) -> bool {
        // g_autoptr is for GObject-derived types, not simple pointers
        // Check if it contains a pointer and is a likely GObject type

        if !type_name.contains('*') {
            return false;
        }

        // Common GObject types that should use g_autoptr
        if type_name.contains("GObject")
            || type_name.contains("GError")
            || type_name.contains("GList")
            || type_name.contains("GSList")
            || type_name.contains("GHashTable")
            || type_name.contains("GBytes")
            || type_name.contains("GVariant")
            || type_name.contains("GArray")
            || type_name.contains("GFile")
            || type_name.contains("GInputStream")
            || type_name.contains("GOutputStream")
        {
            return true;
        }

        // Custom object types (likely if starts with uppercase and contains mixed case)
        if type_name.chars().next().is_some_and(|c| c.is_uppercase())
            && type_name.chars().any(|c| c.is_lowercase())
        {
            return true;
        }

        false
    }

    fn is_var_allocated(&self, statements: &[Statement], var_name: &str) -> bool {
        use gobject_ast::Expression;

        for stmt in statements {
            let mut found = false;
            stmt.walk(&mut |s| {
                match s {
                    // Check init: Type *var = allocation_call()
                    Statement::Declaration(decl) => {
                        if decl.name == var_name
                            && let Some(Expression::Call(call)) = &decl.initializer
                            && call.is_allocation_call()
                        {
                            found = true;
                        }
                    }
                    // Check assignment: var = allocation_call()
                    Statement::Expression(expr_stmt) => {
                        if let Expression::Assignment(assign) = &expr_stmt.expr
                            && assign.lhs == var_name
                            && let Expression::Call(call) = &*assign.rhs
                            && call.is_allocation_call()
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

    fn is_var_manually_freed(&self, statements: &[Statement], var_name: &str) -> bool {
        use gobject_ast::Expression;

        for stmt in statements {
            let mut found = false;
            stmt.walk(&mut |s| {
                if let Statement::Expression(expr_stmt) = s
                    && let Expression::Call(call) = &expr_stmt.expr
                {
                    // Check if this is a cleanup call with our variable
                    if call.is_cleanup_call() && !call.arguments.is_empty() {
                        let gobject_ast::Argument::Expression(arg_expr) = &call.arguments[0];
                        // Check for var or &var
                        if let Some(arg_var) = arg_expr.extract_variable_name()
                            && arg_var == var_name
                        {
                            found = true;
                        }
                    }
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
