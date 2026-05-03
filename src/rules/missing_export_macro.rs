use crate::{
    ast_context::AstContext,
    config::Config,
    rules::{Category, Rule, Violation},
};

pub struct MissingExportMacro;

impl Rule for MissingExportMacro {
    fn name(&self) -> &'static str {
        "missing_export_macro"
    }

    fn description(&self) -> &'static str {
        "Detect public API functions and types without export macros"
    }

    fn long_description(&self) -> Option<&'static str> {
        Some(
            "Detects functions and types in public headers that lack export macros.\n\
             Public API should be marked with appropriate export macros (e.g., G_MODULE_EXPORT, \
             CLUTTER_EXPORT, META_EXPORT) to ensure proper symbol visibility.\n\
             Public API should be marked with appropriate export macros (e.g., G_MODULE_EXPORT, \
             CLUTTER_EXPORT, META_EXPORT) to ensure proper symbol visibility.",
        )
    }

    fn category(&self) -> Category {
        Category::Correctness
    }

    fn requires_meson(&self) -> bool {
        true
    }

    fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        // Only run if we have public header information from meson
        if !ast_context.has_public_private_info() {
            return;
        }

        // Check each file
        for (path, file) in ast_context.iter_all_files() {
            // Skip if not a public header
            if !ast_context.is_public_header(path).unwrap_or(false) {
                continue;
            }

            // Check function declarations
            for func_decl in file.iter_function_declarations() {
                // Skip static functions (not part of public API)
                if func_decl.is_static {
                    continue;
                }

                // Check if function has an export macro
                if func_decl.export_macros.is_empty() {
                    let message = format!(
                        "Public function '{}' in header is missing an export macro (e.g., G_MODULE_EXPORT, *_EXPORT)",
                        func_decl.name
                    );
                    violations.push(self.violation(
                        path,
                        func_decl.location.line,
                        func_decl.location.column,
                        message,
                    ));
                }
            }

            // Check GObject type declarations (G_DECLARE_*)
            for gobject_type in file.iter_all_gobject_types() {
                // Only G_DECLARE_* types go in public headers
                if !gobject_type.kind.is_declare() {
                    continue;
                }

                if gobject_type.export_macros.is_empty() {
                    let message = format!(
                        "'{}' is missing an export macro (e.g., G_MODULE_EXPORT, *_EXPORT)",
                        gobject_type.type_name
                    );
                    violations.push(self.violation(
                        path,
                        gobject_type.location.line,
                        gobject_type.location.column,
                        message,
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_name() {
        let rule = MissingExportMacro;
        assert_eq!(rule.name(), "missing_export_macro");
    }

    #[test]
    fn test_rule_category() {
        let rule = MissingExportMacro;
        assert_eq!(rule.category(), Category::Correctness);
    }
}
