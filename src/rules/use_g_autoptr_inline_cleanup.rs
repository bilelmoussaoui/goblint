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
        for (var_name, (type_info, location)) in &local_vars {
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
                let base_type = &type_info.base_type;

                // Skip if type matches ignore patterns
                if ignore_types.is_match(base_type) {
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

    fn find_local_pointer_vars(
        &self,
        statements: &[Statement],
    ) -> HashMap<String, (gobject_ast::TypeInfo, gobject_ast::SourceLocation)> {
        let mut result = HashMap::new();
        self.collect_local_vars(statements, &mut result);
        result
    }

    fn collect_local_vars(
        &self,
        statements: &[Statement],
        result: &mut HashMap<String, (gobject_ast::TypeInfo, gobject_ast::SourceLocation)>,
    ) {
        for stmt in statements {
            for decl in stmt.iter_declarations() {
                // Skip variables already using g_autoptr/g_autofree
                if decl.type_info.contains("g_autoptr") || decl.type_info.contains("g_autofree") {
                    continue;
                }

                // Track all pointer types that are simple identifiers
                if decl.type_info.is_pointer() && decl.is_simple_identifier() {
                    result.insert(decl.name.clone(), (decl.type_info.clone(), decl.location));
                }
            }
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
        for stmt in statements {
            for call in stmt.iter_calls() {
                if call.is_cleanup_call() && call.arg_contains_variable(0, var_name) {
                    return true;
                }
            }
        }
        false
    }

    fn is_var_freed_with_g_free(&self, statements: &[Statement], var_name: &str) -> bool {
        for stmt in statements {
            for call in stmt.iter_calls() {
                if call.function == "g_free" && call.arg_contains_variable(0, var_name) {
                    return true;
                }
            }
        }
        false
    }

    fn is_var_returned(&self, statements: &[Statement], var_name: &str) -> bool {
        use gobject_ast::Expression;

        for stmt in statements {
            for ret in stmt.iter_returns() {
                if let Some(Expression::Identifier(id)) = &ret.value
                    && id.name == var_name
                {
                    return true;
                }
            }
        }
        false
    }
}
