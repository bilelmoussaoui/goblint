use gobject_ast::Expression;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct SignalEnumCoverage;

impl Rule for SignalEnumCoverage {
    fn name(&self) -> &'static str {
        "signal_enum_coverage"
    }

    fn description(&self) -> &'static str {
        "Ensure all signal enum values have corresponding g_signal_new calls"
    }

    fn category(&self) -> super::Category {
        super::Category::Correctness
    }

    fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        for (path, file) in ast_context.iter_all_files() {
            for enum_info in file.iter_all_enums() {
                if !enum_info.is_signal_enum() {
                    continue;
                }

                // Get all signal enum values (excluding sentinels)
                let signal_values: Vec<&str> = enum_info
                    .values
                    .iter()
                    .filter(|v| !v.is_signal_last())
                    .map(|v| v.name.as_str())
                    .collect();

                if signal_values.is_empty() {
                    continue;
                }

                // Find which class_init corresponds to this enum
                let class_init_func = self.find_class_init_for_enum(file, enum_info);

                if class_init_func.is_none() {
                    // Can't verify coverage without finding class_init
                    continue;
                }

                let class_init = class_init_func.unwrap();

                // Collect all installed signal enum values
                let installed_signals = self.collect_installed_signals(class_init);

                // Check coverage
                for signal_name in &signal_values {
                    if !installed_signals.contains(*signal_name) {
                        violations.push(self.violation(
                            path,
                            enum_info.location.line,
                            1,
                            format!(
                                "Signal enum value '{}' is declared but never installed in {}",
                                signal_name, class_init.name
                            ),
                        ));
                    }
                }
            }
        }
    }
}

impl SignalEnumCoverage {
    /// Find class_init function that corresponds to this signal enum
    fn find_class_init_for_enum<'a>(
        &self,
        file: &'a gobject_ast::FileModel,
        enum_info: &gobject_ast::EnumInfo,
    ) -> Option<&'a gobject_ast::top_level::FunctionDefItem> {
        use gobject_ast::Expression;

        // Get N_SIGNALS name if present
        let n_signals_name = enum_info
            .values
            .last()
            .filter(|v| v.is_signal_last())
            .map(|v| v.name.as_str());

        // Get all signal enum value names (excluding sentinels)
        let signal_names: Vec<&str> = enum_info
            .values
            .iter()
            .filter(|v| !v.is_signal_last())
            .map(|v| v.name.as_str())
            .collect();

        // Find guint signal arrays that use N_SIGNALS or signal names
        let array_names = if let Some(sentinel) = n_signals_name {
            self.find_signal_arrays_for_sentinel(file, sentinel)
        } else {
            Vec::new()
        };

        // Find class_init function that uses this array OR signal names
        for func in file.iter_class_init_functions() {
            let mut uses_signal_enum = false;

            // Check if this class_init uses the array
            if !array_names.is_empty() {
                uses_signal_enum = func.body_statements.iter().any(|stmt| {
                    let mut found = false;
                    stmt.walk(&mut |s| {
                        if let gobject_ast::Statement::Expression(expr_stmt) = s
                            && let Expression::Assignment(assignment) = &expr_stmt.expr
                            && let Expression::Subscript(subscript) = &*assignment.lhs
                            && let Expression::Identifier(id) = &*subscript.array
                            && array_names.contains(&id.name.as_str())
                        {
                            found = true;
                        }
                    });
                    found
                });
            }

            // If no array usage, check if signal names are used in assignments with
            // g_signal_new
            if !uses_signal_enum && !signal_names.is_empty() {
                func.body_statements.iter().for_each(|stmt| {
                    stmt.walk(&mut |s| {
                        if let gobject_ast::Statement::Expression(expr_stmt) = s
                            && let Expression::Assignment(assignment) = &expr_stmt.expr
                            && let Expression::Subscript(subscript) = &*assignment.lhs
                            && let Expression::Identifier(index_id) = &*subscript.index
                            && signal_names.contains(&index_id.name.as_str())
                            && let Expression::Call(call) = &*assignment.rhs
                            && call.function_contains("g_signal_new")
                        {
                            uses_signal_enum = true;
                        }
                    });
                });
            }

            if uses_signal_enum {
                return Some(func);
            }
        }

        None
    }

    /// Find guint signal array names that use the given sentinel
    fn find_signal_arrays_for_sentinel<'a>(
        &self,
        file: &'a gobject_ast::FileModel,
        sentinel_name: &str,
    ) -> Vec<&'a str> {
        use gobject_ast::{
            Statement,
            top_level::{PreprocessorDirective, TopLevelItem},
        };

        let mut array_names = Vec::new();

        fn search_item<'a>(
            item: &'a TopLevelItem,
            sentinel_name: &str,
            array_names: &mut Vec<&'a str>,
        ) {
            match item {
                TopLevelItem::Declaration(Statement::Declaration(decl))
                    if decl.type_info.is_base_type("guint") =>
                {
                    // Check if it's an array declaration using the sentinel name
                    if let Some(Expression::Identifier(size_id)) = &decl.array_size
                        && size_id.name == sentinel_name
                    {
                        array_names.push(&decl.name);
                    }
                }
                TopLevelItem::Preprocessor(PreprocessorDirective::Conditional { body, .. }) => {
                    for nested_item in body {
                        search_item(nested_item, sentinel_name, array_names);
                    }
                }
                _ => {}
            }
        }

        for item in &file.top_level_items {
            search_item(item, sentinel_name, &mut array_names);
        }

        array_names
    }

    /// Collect all signal enum values that are installed in class_init
    fn collect_installed_signals(
        &self,
        class_init: &gobject_ast::top_level::FunctionDefItem,
    ) -> std::collections::HashSet<String> {
        use gobject_ast::Expression;

        let mut installed = std::collections::HashSet::new();

        for stmt in &class_init.body_statements {
            stmt.walk(&mut |s| {
                // Array assignment: signals[SIGNAL_NAME] = g_signal_new(...)
                if let gobject_ast::Statement::Expression(expr_stmt) = s
                    && let Expression::Assignment(assignment) = &expr_stmt.expr
                    && let Expression::Subscript(subscript) = &*assignment.lhs
                    && let Expression::Identifier(enum_id) = &*subscript.index
                    && let Expression::Call(call) = &*assignment.rhs
                    && call.function_contains("g_signal_new")
                {
                    installed.insert(enum_id.name.clone());
                }
            });
        }

        installed
    }
}
