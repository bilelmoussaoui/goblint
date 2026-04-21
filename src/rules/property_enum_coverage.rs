use gobject_ast::Expression;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct PropertyEnumCoverage;

impl Rule for PropertyEnumCoverage {
    fn name(&self) -> &'static str {
        "property_enum_coverage"
    }

    fn description(&self) -> &'static str {
        "Ensure all property enum values have corresponding g_param_spec or g_object_class_override_property"
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
            for enum_info in file.iter_property_enums() {
                // Get all property enum values (excluding PROP_0 and sentinels)
                let property_values: Vec<&str> = enum_info
                    .values
                    .iter()
                    .filter(|v| !v.is_prop_0() && !v.is_prop_last())
                    .map(|v| v.name.as_str())
                    .collect();

                if property_values.is_empty() {
                    continue;
                }

                // Find which class_init corresponds to this enum
                let class_init_func = self.find_class_init_for_enum(file, enum_info);

                if class_init_func.is_none() {
                    // Can't verify coverage without finding class_init
                    continue;
                }

                let class_init = class_init_func.unwrap();

                // Collect all installed property enum values
                let installed_properties = self.collect_installed_properties(class_init);

                // Check coverage
                for prop_name in &property_values {
                    if !installed_properties.contains(*prop_name) {
                        // Find the enum value location for better error reporting
                        // We'll use the enum's location since we don't have per-value line/column
                        violations.push(self.violation(
                            path,
                            enum_info.location.line,
                            1,
                            format!(
                                "Property enum value '{}' is declared but never installed in {}",
                                prop_name, class_init.name
                            ),
                        ));
                    }
                }
            }
        }
    }
}

impl PropertyEnumCoverage {
    /// Find class_init function that corresponds to this property enum
    fn find_class_init_for_enum<'a>(
        &self,
        file: &'a gobject_ast::FileModel,
        enum_info: &gobject_ast::EnumInfo,
    ) -> Option<&'a gobject_ast::top_level::FunctionDefItem> {
        use gobject_ast::Expression;

        // Get N_PROPS name if present
        let n_props_name = enum_info
            .values
            .last()
            .filter(|v| v.is_prop_last())
            .map(|v| v.name.as_str());

        // Get all property enum value names (excluding PROP_0 and sentinels)
        let property_names: Vec<&str> = enum_info
            .values
            .iter()
            .filter(|v| !v.is_prop_0() && !v.is_prop_last())
            .map(|v| v.name.as_str())
            .collect();

        // Find GParamSpec array declarations that use N_PROPS or property names
        let array_names = if let Some(sentinel) = n_props_name {
            self.find_param_spec_arrays_for_sentinel(file, sentinel)
        } else {
            Vec::new()
        };

        // Find class_init function that uses this array OR property names
        for func in file.iter_class_init_functions() {
            let mut uses_property_enum = false;

            // Check if this class_init uses the array
            if !array_names.is_empty() {
                uses_property_enum = func.body_statements.iter().any(|stmt| {
                    let mut found = false;
                    stmt.walk(&mut |s| {
                        if let gobject_ast::Statement::Expression(expr_stmt) = s {
                            match &expr_stmt.expr {
                                Expression::Assignment(assignment) => {
                                    if let Expression::Subscript(subscript) = &*assignment.lhs
                                        && let Expression::Identifier(id) = &*subscript.array
                                        && array_names.contains(&id.name.as_str())
                                    {
                                        found = true;
                                    }
                                }
                                Expression::Call(call)
                                    if call.function_contains("install_properties") =>
                                {
                                    for arg in &call.arguments {
                                        let gobject_ast::Argument::Expression(expr) = arg;
                                        if let Expression::Identifier(ident) = expr.as_ref()
                                            && array_names.contains(&ident.name.as_str())
                                        {
                                            found = true;
                                            break;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    });
                    found
                });
            }

            // If no array usage, check if property names are used
            if !uses_property_enum && !property_names.is_empty() {
                func.body_statements.iter().for_each(|stmt| {
                    stmt.walk(&mut |s| {
                        if let gobject_ast::Statement::Expression(expr_stmt) = s
                            && let Expression::Call(call) = &expr_stmt.expr
                        {
                            // Check install_property or override_property calls
                            if call.function_contains("install_property")
                                || call.function_contains("override_property")
                            {
                                for arg in &call.arguments {
                                    if let gobject_ast::Argument::Expression(expr) = arg
                                        && let Expression::Identifier(ident) = expr.as_ref()
                                        && property_names.contains(&ident.name.as_str())
                                    {
                                        uses_property_enum = true;
                                        break;
                                    }
                                }
                            }
                        }
                    });
                });
            }

            if uses_property_enum {
                return Some(func);
            }
        }

        None
    }

    /// Find GParamSpec array names that use the given sentinel
    fn find_param_spec_arrays_for_sentinel<'a>(
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
                    if decl.type_info.is_base_type("GParamSpec") && decl.type_info.is_pointer() =>
                {
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

    /// Collect all property enum values that are installed in class_init
    fn collect_installed_properties(
        &self,
        class_init: &gobject_ast::top_level::FunctionDefItem,
    ) -> std::collections::HashSet<String> {
        use gobject_ast::Expression;

        let mut installed = std::collections::HashSet::new();

        for stmt in &class_init.body_statements {
            stmt.walk(&mut |s| {
                if let gobject_ast::Statement::Expression(expr_stmt) = s {
                    match &expr_stmt.expr {
                        // Array assignment: obj_props[PROP_NAME] = g_param_spec_*(...)
                        Expression::Assignment(assignment) => {
                            if let Expression::Subscript(subscript) = &*assignment.lhs
                                && let Expression::Identifier(enum_id) = &*subscript.index
                                && let Expression::Call(call) = &*assignment.rhs
                            {
                                // Check if this is a param_spec call
                                if call.function_contains("_param_spec_") {
                                    installed.insert(enum_id.name.clone());
                                }
                            }
                        }
                        // Direct calls: g_object_class_install_property or override_property
                        Expression::Call(call) => {
                            // g_object_class_override_property(class, PROP_NAME, "name")
                            if call.function_contains("override_property") {
                                if let Some(arg) = call.get_arg(1)
                                    && let Expression::Identifier(id) = arg
                                {
                                    installed.insert(id.name.clone());
                                }
                            }
                            // g_object_class_install_property(class, PROP_NAME, spec)
                            else if call.function_contains("install_property")
                                && !call.function_contains("install_properties")
                                && let Some(arg) = call.get_arg(1)
                                && let Expression::Identifier(id) = arg
                            {
                                installed.insert(id.name.clone());
                            }
                        }
                        _ => {}
                    }
                }
            });
        }

        installed
    }
}
