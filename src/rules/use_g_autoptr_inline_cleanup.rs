use std::collections::HashMap;

use globset::{Glob, GlobSet, GlobSetBuilder};

use super::{ConfigOption, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGAutoptrInlineCleanup;

impl Rule for UseGAutoptrInlineCleanup {
    fn name(&self) -> &'static str {
        "use_g_autoptr_inline_cleanup"
    }

    fn description(&self) -> &'static str {
        "Suggest g_autoptr instead of inline manual cleanup (g_object_unref/g_free)"
    }

    fn long_description(&self) -> Option<&'static str> {
        Some(include_str!(
            "../../docs/rules/use_g_autoptr_inline_cleanup.md"
        ))
    }

    fn category(&self) -> super::Category {
        super::Category::Complexity
    }

    fn config_options(&self) -> &'static [ConfigOption] {
        use std::sync::LazyLock;

        static OPTIONS: LazyLock<Vec<ConfigOption>> = LazyLock::new(|| {
            vec![ConfigOption {
                name: "ignore_types",
                option_type: "array<string>",
                default_value: "[]",
                example_value: "[\"cairo_*\", \"Pango*\", \"RsvgHandle\"]",
                description: "List of glob patterns for types to ignore",
            }]
        });

        &OPTIONS
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
        let local_vars: HashMap<String, (gobject_ast::TypeInfo, gobject_ast::SourceLocation)> =
            func.iter_local_declarations()
                .filter(|d| {
                    !d.type_info.uses_auto_cleanup()
                        && d.type_info.is_pointer()
                        && d.is_simple_identifier()
                })
                .map(|d| (d.name.clone(), (d.type_info.clone(), d.location)))
                .collect();

        // For each variable, check if it's a candidate for g_autoptr
        for (var_name, (type_info, location)) in &local_vars {
            // Check if variable is allocated
            let is_allocated = func.is_var_allocated(type_info);

            // Check if variable is manually freed
            let is_manually_freed = func.is_var_passed_to_cleanup(type_info);

            // Check if variable is returned without being freed
            let is_returned = func.is_var_returned(type_info);

            // Check if variable is freed with g_free (should use g_autofree instead)
            let is_freed_with_g_free = func.is_var_passed_to_function(type_info, "g_free", 0);

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
}
