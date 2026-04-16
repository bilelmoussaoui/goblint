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

impl UseGAutoptrGotoCleanup {
    fn check_function(
        &self,
        func: &gobject_ast::FunctionInfo,
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
        for (var_name, (var_type, location)) in &allocated_vars {
            for goto_label in &goto_labels {
                if let Some(cleanup_vars) = cleanup_labels.get(goto_label)
                    && cleanup_vars.contains(var_name)
                {
                    // Extract base type name (strip pointer and qualifiers)
                    let base_type = self.extract_base_type(var_type);
                    violations.push(self.violation(
                        file_path,
                        location.line,
                        location.column,
                        format!(
                            "Consider using g_autoptr({}) {} and g_steal_pointer to avoid goto cleanup",
                            base_type, var_name
                        ),
                    ));
                }
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

    /// Find variables allocated with g_object_new, g_new, etc.
    /// Returns map of var_name -> (type_name, location)
    fn find_allocated_variables(
        &self,
        statements: &[Statement],
    ) -> HashMap<String, (String, gobject_ast::SourceLocation)> {
        let mut result = HashMap::new();

        // First pass: find all local pointer declarations
        let mut local_vars = HashMap::new();
        self.collect_local_pointer_declarations(statements, &mut local_vars);

        // Second pass: find assignments to those variables from allocation functions
        self.collect_allocated_vars(statements, &local_vars, &mut result);

        result
    }

    fn collect_local_pointer_declarations(
        &self,
        statements: &[Statement],
        result: &mut HashMap<String, (String, gobject_ast::SourceLocation)>,
    ) {
        for stmt in statements {
            match stmt {
                Statement::Declaration(decl) => {
                    // Only track pointer types
                    if decl.type_name.contains('*') {
                        // Skip field access names
                        if !decl.name.contains("->") && !decl.name.contains('.') {
                            result.insert(
                                decl.name.clone(),
                                (decl.type_name.clone(), decl.location.clone()),
                            );
                        }
                    }
                }
                Statement::Compound(compound) => {
                    self.collect_local_pointer_declarations(&compound.statements, result);
                }
                Statement::If(if_stmt) => {
                    self.collect_local_pointer_declarations(&if_stmt.then_body, result);
                    if let Some(else_body) = &if_stmt.else_body {
                        self.collect_local_pointer_declarations(else_body, result);
                    }
                }
                Statement::Labeled(labeled) => {
                    self.collect_local_pointer_declarations(
                        std::slice::from_ref(&labeled.statement),
                        result,
                    );
                }
                _ => {}
            }
        }
    }

    fn collect_allocated_vars(
        &self,
        statements: &[Statement],
        local_vars: &HashMap<String, (String, gobject_ast::SourceLocation)>,
        result: &mut HashMap<String, (String, gobject_ast::SourceLocation)>,
    ) {
        use gobject_ast::Expression;

        for stmt in statements {
            match stmt {
                // Pattern 1: Type *var = allocation_call();
                Statement::Declaration(decl) => {
                    if let Some(Expression::Call(call)) = &decl.initializer
                        && self.is_allocation_call(&call.function)
                        && let Some((type_text, location)) = local_vars.get(&decl.name)
                    {
                        result.insert(decl.name.clone(), (type_text.clone(), location.clone()));
                    }
                }
                // Pattern 2: var = allocation_call();
                Statement::Expression(expr_stmt) => {
                    if let Expression::Assignment(assign) = &expr_stmt.expr
                        && let Expression::Call(call) = &*assign.rhs
                        && self.is_allocation_call(&call.function)
                    {
                        // Only simple identifiers, not field expressions
                        if !assign.lhs.contains("->")
                            && !assign.lhs.contains('.')
                            && let Some((type_text, location)) = local_vars.get(&assign.lhs)
                        {
                            result
                                .insert(assign.lhs.clone(), (type_text.clone(), location.clone()));
                        }
                    }
                }
                // Recurse
                Statement::Compound(compound) => {
                    self.collect_allocated_vars(&compound.statements, local_vars, result);
                }
                Statement::If(if_stmt) => {
                    self.collect_allocated_vars(&if_stmt.then_body, local_vars, result);
                    if let Some(else_body) = &if_stmt.else_body {
                        self.collect_allocated_vars(else_body, local_vars, result);
                    }
                }
                Statement::Labeled(labeled) => {
                    self.collect_allocated_vars(
                        std::slice::from_ref(&labeled.statement),
                        local_vars,
                        result,
                    );
                }
                _ => {}
            }
        }
    }

    fn is_allocation_call(&self, func_name: &str) -> bool {
        // Functions that allocate GObject types
        matches!(
            func_name,
            "g_object_new"
                | "g_object_new_with_properties"
                | "g_type_create_instance"
                | "g_new"
                | "g_new0"
                | "g_try_new"
                | "g_try_new0"
                | "g_file_new_for_path"
                | "g_file_new_for_uri"
        ) || func_name.ends_with("_new")
            || func_name.ends_with("_get_instance")
    }

    /// Find all goto statements and collect the labels they target
    fn find_goto_labels(&self, statements: &[Statement]) -> HashSet<String> {
        let mut labels = HashSet::new();
        self.collect_goto_labels(statements, &mut labels);
        labels
    }

    fn collect_goto_labels(&self, statements: &[Statement], labels: &mut HashSet<String>) {
        for stmt in statements {
            match stmt {
                Statement::Goto(goto_stmt) => {
                    labels.insert(goto_stmt.label.clone());
                }
                Statement::Compound(compound) => {
                    self.collect_goto_labels(&compound.statements, labels);
                }
                Statement::If(if_stmt) => {
                    self.collect_goto_labels(&if_stmt.then_body, labels);
                    if let Some(else_body) = &if_stmt.else_body {
                        self.collect_goto_labels(else_body, labels);
                    }
                }
                Statement::Labeled(labeled) => {
                    self.collect_goto_labels(std::slice::from_ref(&labeled.statement), labels);
                }
                _ => {}
            }
        }
    }

    /// Find all labels and what variables they cleanup (unref/free)
    /// Returns map of label_name -> set of variable names
    fn find_cleanup_labels(&self, statements: &[Statement]) -> HashMap<String, HashSet<String>> {
        let mut result = HashMap::new();
        self.collect_cleanup_labels(statements, &mut result);
        result
    }

    fn collect_cleanup_labels(
        &self,
        statements: &[Statement],
        result: &mut HashMap<String, HashSet<String>>,
    ) {
        for stmt in statements {
            match stmt {
                Statement::Labeled(labeled) => {
                    // Find cleanup calls in this labeled statement
                    let mut cleanup_vars = HashSet::new();
                    self.find_cleanup_calls(
                        std::slice::from_ref(&labeled.statement),
                        &mut cleanup_vars,
                    );

                    if !cleanup_vars.is_empty() {
                        result.insert(labeled.label.clone(), cleanup_vars);
                    }

                    // Also recurse to find nested labeled statements
                    self.collect_cleanup_labels(std::slice::from_ref(&labeled.statement), result);
                }
                Statement::Compound(compound) => {
                    self.collect_cleanup_labels(&compound.statements, result);
                }
                Statement::If(if_stmt) => {
                    self.collect_cleanup_labels(&if_stmt.then_body, result);
                    if let Some(else_body) = &if_stmt.else_body {
                        self.collect_cleanup_labels(else_body, result);
                    }
                }
                _ => {}
            }
        }
    }

    fn find_cleanup_calls(&self, statements: &[Statement], cleanup_vars: &mut HashSet<String>) {
        use gobject_ast::Expression;

        for stmt in statements {
            match stmt {
                Statement::Expression(expr_stmt) => {
                    if let Expression::Call(call) = &expr_stmt.expr
                        && self.is_cleanup_call(&call.function)
                        && !call.arguments.is_empty()
                    {
                        let gobject_ast::Argument::Expression(arg_expr) = &call.arguments[0];
                        // Extract variable name (handle &var or var)
                        if let Some(var_name) = arg_expr.extract_variable_name() {
                            cleanup_vars.insert(var_name.to_string());
                        }
                    }
                }
                Statement::Compound(compound) => {
                    self.find_cleanup_calls(&compound.statements, cleanup_vars);
                }
                Statement::If(if_stmt) => {
                    self.find_cleanup_calls(&if_stmt.then_body, cleanup_vars);
                    if let Some(else_body) = &if_stmt.else_body {
                        self.find_cleanup_calls(else_body, cleanup_vars);
                    }
                }
                Statement::Labeled(labeled) => {
                    self.find_cleanup_calls(std::slice::from_ref(&labeled.statement), cleanup_vars);
                }
                _ => {}
            }
        }
    }

    fn is_cleanup_call(&self, func_name: &str) -> bool {
        // Functions that cleanup/free GObject types
        matches!(
            func_name,
            "g_object_unref"
                | "g_clear_object"
                | "g_clear_pointer"
                | "g_error_free"
                | "g_clear_error"
                | "g_free"
                | "g_clear_handle_id"
                | "g_clear_signal_handler"
        ) || func_name.ends_with("_unref")
            || func_name.ends_with("_free")
            || func_name.ends_with("_destroy")
    }
}
