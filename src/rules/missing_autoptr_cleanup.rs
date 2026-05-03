use std::collections::HashSet;

use crate::{
    ast_context::AstContext,
    config::Config,
    rules::{Category, Rule, Violation},
};

pub struct MissingAutoptrCleanup;

impl Rule for MissingAutoptrCleanup {
    fn name(&self) -> &'static str {
        "missing_autoptr_cleanup"
    }

    fn description(&self) -> &'static str {
        "Detect boxed types without G_DEFINE_AUTOPTR_CLEANUP_FUNC"
    }

    fn long_description(&self) -> Option<&'static str> {
        Some(
            "Detects types that don't have automatic g_autoptr() support:\n\
             - Boxed types (G_DEFINE_BOXED_TYPE*) without G_DEFINE_AUTOPTR_CLEANUP_FUNC\n\
             - Old-style GObject types (G_DEFINE_TYPE*) that should use G_DECLARE_* or have explicit cleanup\n\
             Modern GLib code should support g_autoptr() for automatic memory management.",
        )
    }

    fn category(&self) -> Category {
        Category::Style
    }

    fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        // Step 1: Collect all types that need autoptr cleanup
        let mut types_needing_cleanup: Vec<(
            &std::path::Path,
            String,
            gobject_ast::SourceLocation,
            &'static str,
        )> = Vec::new();

        for (path, file) in ast_context.iter_all_files() {
            for gobject_type in file.iter_all_gobject_types() {
                use gobject_ast::model::types::{DefineKind, GObjectTypeKind};

                match &gobject_type.kind {
                    // Boxed and pointer types don't have automatic autoptr support
                    GObjectTypeKind::DefineBoxed { .. }
                    | GObjectTypeKind::Define(DefineKind::Pointer) => {
                        types_needing_cleanup.push((
                            path,
                            gobject_type.type_name.clone(),
                            gobject_type.location,
                            "boxed",
                        ));
                    }
                    // Old-style GObject types without G_DECLARE_* need explicit autoptr
                    GObjectTypeKind::Define(_) => {
                        types_needing_cleanup.push((
                            path,
                            gobject_type.type_name.clone(),
                            gobject_type.location,
                            "old-style",
                        ));
                    }
                    // Modern G_DECLARE_* types have autoptr built-in
                    GObjectTypeKind::Declare { .. } => {
                        // These have autoptr automatically, skip
                    }
                }
            }
        }

        // Step 2: Collect types that have G_DECLARE_* (which includes autoptr)
        let mut declared_types: HashSet<String> = HashSet::new();

        for (_path, file) in ast_context.iter_all_files() {
            for gobject_type in file.iter_all_gobject_types() {
                if gobject_type.kind.is_declare() {
                    declared_types.insert(gobject_type.type_name.clone());
                }
            }
        }

        // Step 3: Collect types with explicit G_DEFINE_AUTOPTR_CLEANUP_FUNC
        let mut autoptr_cleanups: HashSet<String> = HashSet::new();

        for (_path, file) in ast_context.iter_all_files() {
            for item in &file.top_level_items {
                if let gobject_ast::model::top_level::TopLevelItem::Preprocessor(directive) = item
                    && let gobject_ast::model::top_level::PreprocessorDirective::AutoptrCleanupFunc {
                        type_name,
                        ..
                    } = directive
                {
                    autoptr_cleanups.insert(type_name.clone());
                }
            }
        }

        // Step 4: Report violations
        for (path, type_name, location, kind) in types_needing_cleanup {
            // Skip if has G_DECLARE_* (which includes autoptr)
            if declared_types.contains(&type_name) {
                continue;
            }

            // Skip if has explicit G_DEFINE_AUTOPTR_CLEANUP_FUNC
            if autoptr_cleanups.contains(&type_name) {
                continue;
            }

            let message = match kind {
                "boxed" => format!(
                    "Boxed type '{}' is missing G_DEFINE_AUTOPTR_CLEANUP_FUNC macro",
                    type_name
                ),
                "old-style" => format!(
                    "GObject type '{}' defined with G_DEFINE_TYPE* should either use G_DECLARE_* or have G_DEFINE_AUTOPTR_CLEANUP_FUNC",
                    type_name
                ),
                _ => unreachable!(),
            };

            violations.push(self.violation(path, location.line, location.column, message));
        }
    }
}
