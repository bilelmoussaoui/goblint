use gobject_ast::Expression;

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGObjectNotifyByPspec;

impl Rule for UseGObjectNotifyByPspec {
    fn name(&self) -> &'static str {
        "use_g_object_notify_by_pspec"
    }

    fn description(&self) -> &'static str {
        "Suggest g_object_notify_by_pspec instead of g_object_notify for better performance"
    }

    fn category(&self) -> super::Category {
        super::Category::Perf
    }

    fn fixable(&self) -> bool {
        true
    }

    fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        for (path, file) in ast_context.iter_all_files() {
            let source = &file.source;

            // Build a map of property names to (enum_value, array_name, class_prefix) for
            // this file
            let property_map = self.build_property_map(file, source);

            // Find all g_object_notify calls
            for func in file.iter_function_definitions() {
                for call in func.find_calls(&["g_object_notify"]) {
                    self.check_call(path, call, source, &property_map, func, violations);
                }
            }
        }
    }
}

impl UseGObjectNotifyByPspec {
    fn check_call(
        &self,
        file_path: &std::path::Path,
        call: &gobject_ast::CallExpression,
        source: &[u8],
        property_map: &std::collections::HashMap<String, Vec<(String, String, String)>>,
        func: &gobject_ast::top_level::FunctionDefItem,
        violations: &mut Vec<Violation>,
    ) {
        // Need exactly 2 arguments: object and property name
        if call.arguments.len() != 2 {
            return;
        }

        // Check if second argument is a string literal
        let Some(property_expr) = call.get_arg(1) else {
            return;
        };
        if !property_expr.is_string_literal() {
            return;
        }

        // Get the string literal value
        let Expression::StringLiteral(string_lit) = property_expr else {
            unreachable!();
        };

        let property_name = string_lit.value.trim_matches('"');

        // Collect all unique array names from the property map
        let array_names: Vec<String> = property_map
            .values()
            .flatten()
            .map(|(_, array_name, _)| array_name.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // Look up the property in our map
        if let Some(candidates) = property_map.get(property_name) {
            // Try to disambiguate by matching object type to class prefix
            let disambiguated = if candidates.len() > 1 {
                self.disambiguate_by_type(call, source, func, candidates)
            } else {
                Some(&candidates[0])
            };

            if let Some((enum_value, array_name, _)) = disambiguated {
                // Get the object expression (first argument)
                let Some(obj_expr) = call.get_arg(0) else {
                    return;
                };
                let Some(obj_str) = obj_expr.to_source_string(source) else {
                    return;
                };

                // Generate fix: replace entire call
                let replacement = format!(
                    "g_object_notify_by_pspec ({}, {}[{}])",
                    obj_str, array_name, enum_value
                );

                violations.push(self.violation_with_fix(
                    file_path,
                    call.location.line,
                    call.location.column,
                    format!(
                        "Use g_object_notify_by_pspec({}, {}[{}]) instead of g_object_notify({}, \"{}\") for better performance",
                        obj_str, array_name, enum_value, obj_str, property_name
                    ),
                    Fix::new(call.location.start_byte, call.location.end_byte, replacement),
                ));
            } else {
                // Still ambiguous after type checking, just suggest without fix
                let property_constant = self.property_name_to_constant(property_name);
                let array_name = &candidates[0].1; // Use first array name as example
                violations.push(self.violation(
                    file_path,
                    call.location.line,
                    call.location.column,
                    format!(
                        "Use g_object_notify_by_pspec(obj, {}[{}]) instead of g_object_notify(obj, \"{}\") for better performance (ambiguous: multiple classes define this property)",
                        array_name, property_constant, property_name
                    ),
                ));
            }
        } else {
            // No GParamSpec array found for this property
            let property_constant = self.property_name_to_constant(property_name);

            // Use actual array name if we have any, otherwise generic "properties"
            let suggested_array = if !array_names.is_empty() {
                &array_names[0]
            } else {
                "properties"
            };

            violations.push(self.violation(
                file_path,
                call.location.line,
                call.location.column,
                format!(
                    "Use g_object_notify_by_pspec(obj, {}[{}]) instead of g_object_notify(obj, \"{}\") for better performance",
                    suggested_array, property_constant, property_name
                ),
            ));
        }
    }

    /// Build a map of property names to Vec<(enum_value, array_name,
    /// class_prefix)> Multiple entries indicate ambiguity (same property
    /// name in multiple classes)
    fn build_property_map(
        &self,
        file: &gobject_ast::FileModel,
        source: &[u8],
    ) -> std::collections::HashMap<String, Vec<(String, String, String)>> {
        use gobject_ast::Statement;
        let mut map: std::collections::HashMap<String, Vec<(String, String, String)>> =
            std::collections::HashMap::new();

        // Find all class_init functions
        for func in file.iter_function_definitions() {
            if !func.name.ends_with("_class_init")
                && !func.name.ends_with("_class_install_properties")
            {
                continue;
            }

            // Extract class prefix from function name (e.g., "foo_class_init" -> "foo")
            let class_prefix = self.extract_class_prefix(&func.name);

            // Look for g_object_class_install_properties calls to get the array name
            let array_names: Vec<String> = func
                .find_calls(&["g_object_class_install_properties"])
                .iter()
                .filter_map(|call| call.get_arg(2).and_then(|arg| arg.to_source_string(source)))
                .collect();

            if array_names.is_empty() {
                continue;
            }

            // Walk through all statements looking for props[PROP_X] = g_param_spec_*()
            // patterns
            for stmt in &func.body_statements {
                if let Statement::Expression(expr_stmt) = stmt
                    && let Expression::Assignment(assignment) = &expr_stmt.expr
                {
                    // LHS should be array subscript like props[PROP_X]
                    if let Expression::Subscript(subscript) = &*assignment.lhs
                        && let Some(array_name) = subscript.array.to_source_string(source)
                    {
                        // Check if this array is one we're tracking
                        if !array_names.contains(&array_name) {
                            continue;
                        }

                        // Get the enum value (the subscript index)
                        if let Some(enum_value) = subscript.index.to_source_string(source) {
                            // RHS should be a g_param_spec_* call
                            if let Expression::Call(param_call) = &*assignment.rhs {
                                let func_name = param_call.function_name();
                                if func_name.contains("_param_spec_") {
                                    // Extract property name from first argument
                                    if let Some(name_arg) = param_call.get_arg(0)
                                        && let Expression::StringLiteral(name_lit) = name_arg
                                    {
                                        let prop_name = name_lit.value.trim_matches('"');
                                        map.entry(prop_name.to_string()).or_default().push((
                                            enum_value,
                                            array_name,
                                            class_prefix.clone(),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        map
    }

    /// Disambiguate by matching the object type to the class prefix
    fn disambiguate_by_type<'a>(
        &self,
        call: &gobject_ast::CallExpression,
        source: &[u8],
        func: &gobject_ast::top_level::FunctionDefItem,
        candidates: &'a [(String, String, String)],
    ) -> Option<&'a (String, String, String)> {
        // Get the object expression (first argument)
        let obj_expr = call.get_arg(0)?;
        let obj_str = obj_expr.to_source_string(source)?;

        // Strip casts like G_OBJECT(self) to get the base identifier
        let obj_identifier = self.extract_identifier(&obj_str);

        // Find which function parameter matches this identifier
        let param_type = func
            .parameters
            .iter()
            .find(|p| {
                p.name
                    .as_ref()
                    .map(|n| n == &obj_identifier)
                    .unwrap_or(false)
            })
            .map(|p| &p.type_info.base_type)?;

        // Extract class prefix from type (e.g., "FooObject" -> "foo")
        let type_prefix = self.extract_type_prefix(param_type);

        // Find candidate that matches this prefix
        candidates
            .iter()
            .find(|(_, _, class_prefix)| *class_prefix == type_prefix)
    }

    /// Extract class prefix from function name
    /// e.g., "foo_class_init" -> "foo", "bar_class_install_properties" -> "bar"
    fn extract_class_prefix(&self, func_name: &str) -> String {
        if let Some(pos) = func_name.find("_class_init") {
            func_name[..pos].to_string()
        } else if let Some(pos) = func_name.find("_class_install_properties") {
            func_name[..pos].to_string()
        } else {
            func_name.to_string()
        }
    }

    /// Extract identifier from expression like "G_OBJECT(self)" -> "self"
    fn extract_identifier(&self, expr: &str) -> String {
        let trimmed = expr.trim();

        // Handle casts like G_OBJECT(self) or (GObject*)self
        if let Some(start) = trimmed.rfind('(')
            && let Some(end) = trimmed.rfind(')')
        {
            return trimmed[start + 1..end].trim().to_string();
        }

        trimmed.to_string()
    }

    /// Extract class prefix from type name
    /// e.g., "FooObject" -> "foo", "BarClass" -> "bar", "BazThing *" ->
    /// "baz_thing"
    fn extract_type_prefix(&self, type_name: &str) -> String {
        let trimmed = type_name
            .trim()
            .trim_end_matches('*')
            .trim_end_matches("Object")
            .trim_end_matches("Class")
            .trim();

        // Convert CamelCase to snake_case
        let mut result = String::new();
        for (i, ch) in trimmed.chars().enumerate() {
            if ch.is_uppercase() && i > 0 {
                result.push('_');
            }
            result.push(ch.to_ascii_lowercase());
        }

        result
    }

    /// Convert property-name to PROP_NAME constant style
    fn property_name_to_constant(&self, property_name: &str) -> String {
        // Convert kebab-case or camelCase to UPPER_SNAKE_CASE
        let mut result = String::with_capacity(property_name.len() + 5);
        result.push_str("PROP_");

        for c in property_name.chars() {
            if c == '-' {
                result.push('_');
            } else {
                result.push(c.to_ascii_uppercase());
            }
        }

        result
    }
}
