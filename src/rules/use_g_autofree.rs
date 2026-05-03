use std::collections::HashMap;

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
        let local_vars: HashMap<String, (gobject_ast::TypeInfo, gobject_ast::SourceLocation)> =
            func.iter_local_declarations()
                .filter(|d| !d.type_info.uses_auto_cleanup() && d.is_simple_identifier())
                .map(|d| (d.name.clone(), (d.type_info.clone(), d.location)))
                .collect();

        // For each variable, check if it's a candidate for g_autofree
        for (var_name, (type_info, location)) in &local_vars {
            // Only suggest g_autofree for simple types (char*, guint8*, void*, etc.)
            // Not for GObject* types (those should use g_autoptr)
            if !type_info.is_pointer() {
                continue;
            }

            // Check if variable is allocated
            let is_allocated = func.is_var_allocated_with(type_info, |call| {
                call.function_name_str()
                    .is_some_and(|name| self.is_autofree_allocation(name))
            });

            // Check if variable is manually freed
            let is_manually_freed = func.is_var_passed_to_function(type_info, "g_free", 0);

            // Check if variable is returned
            let is_returned = func.is_var_returned(type_info);

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
}
