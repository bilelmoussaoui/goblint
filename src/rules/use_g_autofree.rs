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
        func: &gobject_ast::FunctionInfo,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        if !func.is_definition {
            return;
        }

        // Find all local pointer declarations
        let local_vars = self.find_local_pointer_vars(&func.body_statements);

        // For each variable, check if it's a candidate for g_autofree
        for (var_name, (var_type, location)) in &local_vars {
            // Only suggest g_autofree for simple types (char*, guint8*, void*, etc.)
            // Not for GObject* types (those should use g_autoptr)
            if !self.is_autofree_candidate(var_type) {
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
    fn is_autofree_candidate(&self, var_type: &str) -> bool {
        let type_lower = var_type.to_lowercase();

        // g_autofree is for simple pointer types: char*, guint8*, void*, etc.
        // Not for GObject-derived types (those use g_autoptr)

        // Simple types that should use g_autofree
        if type_lower.contains("char")
            || type_lower.contains("guint8")
            || type_lower.contains("gint8")
            || type_lower.contains("guchar")
            || type_lower.contains("gchar")
            || type_lower.contains("uint8_t")
            || type_lower.contains("int8_t")
            || type_lower.contains("void")
        {
            return true;
        }

        // Skip GObject types - these should use g_autoptr instead
        // Common GObject patterns: GType*, GObject*, anything with G[A-Z][a-z]*
        if var_type.contains("GError")
            || var_type.contains("GObject")
            || var_type.contains("GList")
            || var_type.contains("GSList")
            || var_type.contains("GHashTable")
            || var_type.contains("GBytes")
            || var_type.contains("GVariant")
            || var_type.contains("GArray")
        {
            return false;
        }

        // Check for custom types (like CoglTexture, MetaWindow, etc.)
        // These should use g_autoptr
        if var_type.chars().next().is_some_and(|c| c.is_uppercase()) {
            // If starts with uppercase and contains mixed case, likely an object type
            if var_type.chars().any(|c| c.is_lowercase()) {
                return false;
            }
        }

        false
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
                    // Skip if var name contains -> or . (field access)
                    if !decl.name.contains("->") && !decl.name.contains('.') {
                        result.insert(
                            decl.name.clone(),
                            (decl.type_name.clone(), decl.location.clone()),
                        );
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

    fn is_var_allocated(&self, statements: &[Statement], var_name: &str) -> bool {
        self.find_var_allocation(statements, var_name)
    }

    fn find_var_allocation(&self, statements: &[Statement], var_name: &str) -> bool {
        use gobject_ast::Expression;

        for stmt in statements {
            match stmt {
                // Check init: char *var = g_strdup(...)
                Statement::Declaration(decl) => {
                    if decl.name == var_name
                        && let Some(Expression::Call(call)) = &decl.initializer
                        && self.is_autofree_allocation(&call.function)
                    {
                        return true;
                    }
                }
                // Check assignment: var = g_strdup(...)
                Statement::Expression(expr_stmt) => {
                    if let Expression::Assignment(assign) = &expr_stmt.expr
                        && assign.lhs == var_name
                        && let Expression::Call(call) = &*assign.rhs
                        && self.is_autofree_allocation(&call.function)
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
        )
    }

    fn is_var_manually_freed(&self, statements: &[Statement], var_name: &str) -> bool {
        self.find_manual_free(statements, var_name)
    }

    fn find_manual_free(&self, statements: &[Statement], var_name: &str) -> bool {
        use gobject_ast::Expression;

        for stmt in statements {
            match stmt {
                Statement::Expression(expr_stmt) => {
                    if let Expression::Call(call) = &expr_stmt.expr
                        && call.function == "g_free"
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
