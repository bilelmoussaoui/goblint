use std::collections::HashMap;

use globset::{Glob, GlobSet, GlobSetBuilder};
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
        config: &Config,
        func: &gobject_ast::top_level::FunctionDefItem,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        // Build ignore matcher from config
        let ignore_types = self.build_ignore_types_matcher(config);
        self.check_function(func, path, violations, &ignore_types);
    }
}

impl UseGAutoptrInlineCleanup {
    /// Build a GlobSet matcher for types to ignore from config
    fn build_ignore_types_matcher(&self, config: &Config) -> GlobSet {
        let mut builder = GlobSetBuilder::new();

        if let Some(rule_config) = config.get_rule_config(self.name())
            && let Some(toml::Value::Array(patterns)) = rule_config.options.get("ignore_types")
        {
            for pattern in patterns {
                if let toml::Value::String(s) = pattern
                    && let Ok(glob) = Glob::new(s)
                {
                    builder.add(glob);
                }
            }
        }

        builder.build().unwrap_or_else(|_| GlobSet::empty())
    }

    fn check_function(
        &self,
        func: &gobject_ast::top_level::FunctionDefItem,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
        ignore_types: &GlobSet,
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

            // Check if variable is freed with g_free (should use g_autofree instead)
            let is_freed_with_g_free =
                self.is_var_freed_with_g_free(&func.body_statements, var_name);

            // Suggest g_autoptr if:
            // 1. Variable is allocated
            // 2. Variable is manually freed at least once
            // 3. Variable is not returned directly (would need g_steal_pointer)
            // 4. Variable is NOT freed with g_free (those should use g_autofree)
            if is_allocated && is_manually_freed && !is_returned && !is_freed_with_g_free {
                let base_type = self.extract_base_type(var_type);

                // Skip if type matches ignore patterns
                if ignore_types.is_match(&base_type) {
                    continue;
                }

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

                    // Track all pointer types - we'll filter later based on cleanup function
                    if decl.type_name.contains('*') {
                        // Skip field access names
                        if !decl.name.contains("->") && !decl.name.contains('.') {
                            result
                                .insert(decl.name.clone(), (decl.type_name.clone(), decl.location));
                        }
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
                    // Check if this is a cleanup call with our variable
                    && call.is_cleanup_call()
                    && let Some(arg_expr) = call.get_arg(0)
                    // Check for var or &var
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

    fn is_var_freed_with_g_free(&self, statements: &[Statement], var_name: &str) -> bool {
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
