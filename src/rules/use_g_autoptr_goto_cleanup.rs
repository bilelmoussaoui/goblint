use std::collections::{HashMap, HashSet};

use gobject_ast::Statement;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGAutoptrGotoCleanup;

impl Rule for UseGAutoptrGotoCleanup {
    fn name(&self) -> &'static str {
        "use_g_autoptr_goto_cleanup"
    }

    fn description(&self) -> &'static str {
        "Suggest g_autoptr instead of goto error cleanup pattern"
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

impl UseGAutoptrGotoCleanup {
    fn check_function(
        &self,
        func: &gobject_ast::top_level::FunctionDefItem,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        // Find all allocated variables (g_object_new, g_new, etc.)
        let allocated_vars = self.find_allocated_variables(&func.body_statements);

        // Find all goto statements and the labels they target
        let goto_labels = self.find_goto_labels(&func.body_statements);

        // Find cleanup labels (labels that unref/free variables)
        let cleanup_labels = self.find_cleanup_labels(&func.body_statements);

        // Match: if allocated var has goto to cleanup label that frees it
        for (var_name, (type_info, location)) in &allocated_vars {
            for goto_label in &goto_labels {
                if let Some(cleanup_vars) = cleanup_labels.get(goto_label)
                    && cleanup_vars.contains(var_name)
                {
                    // Extract base type name (strip pointer and qualifiers)
                    violations.push(self.violation(
                        file_path,
                        location.line,
                        location.column,
                        format!(
                            "Consider using g_autoptr({}) {} and g_steal_pointer to avoid goto cleanup",
                            type_info.base_type, var_name
                        ),
                    ));
                }
            }
        }
    }

    /// Find variables allocated with g_object_new, g_new, etc.
    /// Returns map of var_name -> (type_name, location)
    fn find_allocated_variables(
        &self,
        statements: &[Statement],
    ) -> HashMap<String, (gobject_ast::TypeInfo, gobject_ast::SourceLocation)> {
        let mut result = HashMap::new();

        let local_vars: HashMap<String, (gobject_ast::TypeInfo, gobject_ast::SourceLocation)> =
            statements
                .iter()
                .flat_map(|s| s.iter_declarations())
                .filter(|d| {
                    !d.type_info.uses_auto_cleanup()
                        && d.type_info.is_pointer()
                        && d.is_simple_identifier()
                })
                .map(|d| (d.name.clone(), (d.type_info.clone(), d.location)))
                .collect();

        // Second pass: find assignments to those variables from allocation functions
        self.collect_allocated_vars(statements, &local_vars, &mut result);

        result
    }

    fn collect_allocated_vars(
        &self,
        statements: &[Statement],
        local_vars: &HashMap<String, (gobject_ast::TypeInfo, gobject_ast::SourceLocation)>,
        result: &mut HashMap<String, (gobject_ast::TypeInfo, gobject_ast::SourceLocation)>,
    ) {
        use gobject_ast::Expression;

        for stmt in statements {
            stmt.walk(&mut |s| {
                match s {
                    // Pattern 1: Type *var = allocation_call();
                    Statement::Declaration(decl) => {
                        if let Some(Expression::Call(call)) = &decl.initializer
                            && call.is_allocation_call()
                            && let Some((type_info, location)) = local_vars.get(&decl.name)
                        {
                            result.insert(decl.name.clone(), (type_info.clone(), *location));
                        }
                    }
                    // Pattern 2: var = allocation_call();
                    Statement::Expression(expr_stmt) => {
                        if let Expression::Assignment(assign) = &expr_stmt.expr
                            && let Expression::Call(call) = &*assign.rhs
                            && call.is_allocation_call()
                            // Only simple identifiers, not field expressions
                            && let Expression::Identifier(id) = &*assign.lhs
                            && let Some((type_info, location)) = local_vars.get(&id.name)
                        {
                            result.insert(id.name.clone(), (type_info.clone(), *location));
                        }
                    }
                    _ => {}
                }
            });
        }
    }

    /// Find all goto statements and collect the labels they target
    fn find_goto_labels(&self, statements: &[Statement]) -> HashSet<String> {
        let mut labels = HashSet::new();
        for stmt in statements {
            stmt.walk(&mut |s| {
                if let Statement::Goto(goto_stmt) = s {
                    labels.insert(goto_stmt.label.clone());
                }
            });
        }
        labels
    }

    /// Find all labels and what variables they cleanup (unref/free)
    /// Returns map of label_name -> set of variable names
    fn find_cleanup_labels(&self, statements: &[Statement]) -> HashMap<String, HashSet<String>> {
        let mut result = HashMap::new();

        for stmt in statements {
            stmt.walk(&mut |s| {
                if let Statement::Labeled(labeled) = s {
                    // Find cleanup calls in this labeled statement
                    let cleanup_vars = self.find_cleanup_calls(&labeled.statement);
                    if !cleanup_vars.is_empty() {
                        result.insert(labeled.label.clone(), cleanup_vars);
                    }
                }
            });
        }

        result
    }

    fn find_cleanup_calls(&self, stmt: &Statement) -> HashSet<String> {
        let mut cleanup_vars = HashSet::new();
        for call in stmt.iter_calls() {
            if call.is_cleanup_call()
                && let Some(arg_expr) = call.get_arg(0)
                // Extract variable name (handle &var or var)
                && let Some(var_name) = arg_expr.extract_variable_name()
            {
                cleanup_vars.insert(var_name.to_string());
            }
        }
        cleanup_vars
    }
}
