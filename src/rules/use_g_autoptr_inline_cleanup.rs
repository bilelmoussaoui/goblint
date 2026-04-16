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
            match stmt {
                Statement::Declaration(decl) => {
                    // Skip variables already using g_autoptr/g_autofree
                    if decl.type_name.contains("g_autoptr") || decl.type_name.contains("g_autofree")
                    {
                        continue;
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
                Statement::Compound(compound) => {
                    self.collect_local_vars(&compound.statements, result);
                }
                Statement::If(if_stmt) => {
                    self.collect_local_vars(&if_stmt.then_body, result);
                    if let Some(else_body) = &if_stmt.else_body {
                        self.collect_local_vars(else_body, result);
                    }
                }
                Statement::Labeled(labeled) => {
                    self.collect_local_vars(std::slice::from_ref(&labeled.statement), result);
                }
                _ => {}
            }
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
        self.find_var_allocation(statements, var_name)
    }

    fn find_var_allocation(&self, statements: &[Statement], var_name: &str) -> bool {
        use gobject_ast::Expression;

        for stmt in statements {
            match stmt {
                // Check init: Type *var = allocation_call()
                Statement::Declaration(decl) => {
                    if decl.name == var_name
                        && let Some(Expression::Call(call)) = &decl.initializer
                        && self.is_allocation_call(&call.function)
                    {
                        return true;
                    }
                }
                // Check assignment: var = allocation_call()
                Statement::Expression(expr_stmt) => {
                    if let Expression::Assignment(assign) = &expr_stmt.expr
                        && assign.lhs == var_name
                        && let Expression::Call(call) = &*assign.rhs
                        && self.is_allocation_call(&call.function)
                    {
                        return true;
                    }
                }
                // Recurse
                Statement::Compound(compound) => {
                    if self.find_var_allocation(&compound.statements, var_name) {
                        return true;
                    }
                }
                Statement::If(if_stmt) => {
                    if self.find_var_allocation(&if_stmt.then_body, var_name) {
                        return true;
                    }
                    if let Some(else_body) = &if_stmt.else_body
                        && self.find_var_allocation(else_body, var_name)
                    {
                        return true;
                    }
                }
                Statement::Labeled(labeled) => {
                    if self.find_var_allocation(std::slice::from_ref(&labeled.statement), var_name)
                    {
                        return true;
                    }
                }
                _ => {}
            }
        }

        false
    }

    fn is_allocation_call(&self, func_name: &str) -> bool {
        // Functions that allocate GObject types
        matches!(
            func_name,
            "g_object_new"
                | "g_object_new_with_properties"
                | "g_type_create_instance"
                | "g_file_new_for_path"
                | "g_file_new_for_uri"
                | "g_file_new_tmp"
                | "g_variant_new"
                | "g_variant_ref_sink"
                | "g_bytes_new"
                | "g_bytes_new_take"
                | "g_hash_table_new"
                | "g_hash_table_new_full"
                | "g_array_new"
                | "g_ptr_array_new"
                | "g_error_new"
                | "g_error_new_literal"
        ) || func_name.ends_with("_new")
            || func_name.ends_with("_get_instance")
    }

    fn is_var_manually_freed(&self, statements: &[Statement], var_name: &str) -> bool {
        self.find_manual_free(statements, var_name)
    }

    fn find_manual_free(&self, statements: &[Statement], var_name: &str) -> bool {
        use gobject_ast::Expression;

        for stmt in statements {
            match stmt {
                Statement::Expression(expr_stmt) => {
                    if let Expression::Call(call) = &expr_stmt.expr {
                        // Check if this is a cleanup call with our variable
                        if self.is_cleanup_call(&call.function) && !call.arguments.is_empty() {
                            let gobject_ast::Argument::Expression(arg_expr) = &call.arguments[0];
                            // Check for var or &var
                            if let Some(arg_var) = arg_expr.extract_variable_name()
                                && arg_var == var_name
                            {
                                return true;
                            }
                        }
                    }
                }
                // Recurse
                Statement::Compound(compound) => {
                    if self.find_manual_free(&compound.statements, var_name) {
                        return true;
                    }
                }
                Statement::If(if_stmt) => {
                    if self.find_manual_free(&if_stmt.then_body, var_name) {
                        return true;
                    }
                    if let Some(else_body) = &if_stmt.else_body
                        && self.find_manual_free(else_body, var_name)
                    {
                        return true;
                    }
                }
                Statement::Labeled(labeled) => {
                    if self.find_manual_free(std::slice::from_ref(&labeled.statement), var_name) {
                        return true;
                    }
                }
                _ => {}
            }
        }

        false
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
                | "g_list_free"
                | "g_list_free_full"
                | "g_slist_free"
                | "g_slist_free_full"
                | "g_hash_table_unref"
                | "g_hash_table_destroy"
                | "g_bytes_unref"
                | "g_variant_unref"
                | "g_array_unref"
                | "g_array_free"
                | "g_ptr_array_unref"
                | "g_ptr_array_free"
        ) || func_name.ends_with("_unref")
            || func_name.ends_with("_free")
            || func_name.ends_with("_destroy")
    }

    fn is_var_returned(&self, statements: &[Statement], var_name: &str) -> bool {
        self.find_return_of_var(statements, var_name)
    }

    fn find_return_of_var(&self, statements: &[Statement], var_name: &str) -> bool {
        use gobject_ast::Expression;

        for stmt in statements {
            match stmt {
                Statement::Return(ret) => {
                    if let Some(Expression::Identifier(id)) = &ret.value
                        && id.name == var_name
                    {
                        return true;
                    }
                }
                // Recurse
                Statement::Compound(compound) => {
                    if self.find_return_of_var(&compound.statements, var_name) {
                        return true;
                    }
                }
                Statement::If(if_stmt) => {
                    if self.find_return_of_var(&if_stmt.then_body, var_name) {
                        return true;
                    }
                    if let Some(else_body) = &if_stmt.else_body
                        && self.find_return_of_var(else_body, var_name)
                    {
                        return true;
                    }
                }
                Statement::Labeled(labeled) => {
                    if self.find_return_of_var(std::slice::from_ref(&labeled.statement), var_name) {
                        return true;
                    }
                }
                _ => {}
            }
        }

        false
    }
}
