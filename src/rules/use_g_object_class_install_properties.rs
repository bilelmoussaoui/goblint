use gobject_ast::{Expression, Statement};

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGObjectClassInstallProperties;

impl Rule for UseGObjectClassInstallProperties {
    fn name(&self) -> &'static str {
        "use_g_object_class_install_properties"
    }

    fn description(&self) -> &'static str {
        "Suggest g_object_class_install_properties for multiple g_object_class_install_property calls"
    }

    fn category(&self) -> super::Category {
        super::Category::Complexity
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
            // Find all class_init or class-related functions
            for func in file.iter_class_init_functions() {
                // Find all g_object_class_install_property calls
                let install_property_calls = func.find_calls(&["g_object_class_install_property"]);

                if install_property_calls.is_empty() {
                    continue;
                }

                // Try to find property enum to generate fixes
                let fixes =
                    self.try_generate_fixes(file, func, &install_property_calls, &file.source);

                let first_call = install_property_calls[0];
                let message = if fixes.is_empty() {
                    format!(
                        "Consider using g_object_class_install_properties() instead of {} g_object_class_install_property() calls",
                        install_property_calls.len()
                    )
                } else {
                    format!(
                        "Use g_object_class_install_properties() instead of {} g_object_class_install_property() calls",
                        install_property_calls.len()
                    )
                };

                violations.push(self.violation_with_fixes(
                    path,
                    first_call.location.line,
                    first_call.location.column,
                    message,
                    fixes,
                ));
            }
        }
    }
}

impl UseGObjectClassInstallProperties {
    /// Find the property enum used by the install_property calls
    fn find_property_enum<'a>(
        &self,
        file: &'a gobject_ast::FileModel,
        install_calls: &[&gobject_ast::CallExpression],
        source: &[u8],
    ) -> Option<&'a gobject_ast::EnumInfo> {
        // Extract property IDs from install_property calls
        let mut prop_ids = Vec::new();
        for call in install_calls {
            if let Some(prop_id_arg) = call.get_arg(1)
                && let Some(prop_id) = prop_id_arg.to_source_string(source)
            {
                prop_ids.push(prop_id);
            }
        }

        // Find enum that contains these property IDs
        for enum_info in file.iter_all_enums() {
            let enum_value_names: Vec<_> =
                enum_info.values.iter().map(|v| v.name.as_str()).collect();

            // Check if at least some property IDs are in this enum
            let matches = prop_ids
                .iter()
                .filter(|pid| enum_value_names.contains(&pid.as_str()))
                .count();
            if matches >= prop_ids.len() / 2 {
                return Some(enum_info);
            }
        }

        None
    }

    /// Try to generate fixes if we can find the property enum
    fn try_generate_fixes(
        &self,
        file: &gobject_ast::FileModel,
        class_init: &gobject_ast::top_level::FunctionDefItem,
        install_calls: &[&gobject_ast::CallExpression],
        source: &[u8],
    ) -> Vec<Fix> {
        // Try to find the property enum used by these calls
        let Some(property_enum) = self.find_property_enum(file, install_calls, source) else {
            return Vec::new(); // Can't generate fixes without enum
        };

        self.generate_fixes(file, class_init, install_calls, property_enum, source)
    }

    fn generate_fixes(
        &self,
        file: &gobject_ast::FileModel,
        class_init: &gobject_ast::top_level::FunctionDefItem,
        install_calls: &[&gobject_ast::CallExpression],
        property_enum: &gobject_ast::EnumInfo,
        source: &[u8],
    ) -> Vec<Fix> {
        let mut fixes = Vec::new();

        // Pre-collect all param_spec assignments (variable pattern)
        let param_spec_assignments: Vec<_> = class_init
            .find_param_spec_assignments(source)
            .into_iter()
            .filter_map(|a| {
                if let gobject_ast::ParamSpecAssignment::Variable {
                    variable_name,
                    property_name,
                    statement_location,
                    call,
                    ..
                } = a
                {
                    Some((variable_name, property_name, statement_location, call))
                } else {
                    None
                }
            })
            .collect();

        // Check if enum has N_PROPS sentinel
        let n_props_sentinel = property_enum.values.iter().find(|v| v.is_prop_last());
        let n_props_name = if let Some(sentinel) = n_props_sentinel {
            sentinel.name.clone()
        } else {
            // Need to add N_PROPS to the enum
            let sentinel_name = self.determine_n_props_name(property_enum);

            // Insert N_PROPS after the last enum value
            let last_value = property_enum.values.last().unwrap();
            // Use the same indentation as the last enum value
            let value_indentation = last_value.location.extract_indentation(source);

            // Check if there's a comma at end_byte (some parsers include it, some don't)
            let (insertion_pos, needs_comma) = if last_value.location.end_byte < source.len()
                && source[last_value.location.end_byte] == b','
            {
                // Comma is at end_byte, insert after it
                (last_value.location.end_byte + 1, false)
            } else {
                // No comma at end_byte, we need to add one
                (last_value.location.end_byte, true)
            };

            let n_props_decl = if needs_comma {
                format!(",\n{}{}", value_indentation, sentinel_name)
            } else {
                format!("\n{}{}", value_indentation, sentinel_name)
            };

            fixes.push(Fix::new(insertion_pos, insertion_pos, n_props_decl));

            sentinel_name
        };

        // Determine array name: prefer "props", fallback to "obj_props"
        let array_name = self.determine_array_name(file, source);

        // Fix: Add GParamSpec array declaration after the enum
        // For non-typedef enums, end_byte may point AT the semicolon rather than after
        // it So we need to skip past it if present
        let insertion_pos = if property_enum.location.end_byte < source.len()
            && source[property_enum.location.end_byte] == b';'
        {
            property_enum.location.end_byte + 1
        } else {
            property_enum.location.end_byte
        };

        let array_decl = format!(
            "\n\nstatic GParamSpec *{}[{}] = {{ NULL, }};",
            array_name, n_props_name
        );
        fixes.push(Fix::new(insertion_pos, insertion_pos, array_decl));

        // Find the GObjectClass declaration to get object_class variable name and
        // indentation
        let object_class_var = self
            .find_object_class_variable(class_init)
            .unwrap_or_else(|| "object_class".to_string());

        // Get indentation for the install_properties call
        let indentation = if let Some(first_call) = install_calls.first() {
            if let Some(stmt) = self.find_statement_containing_call(
                &class_init.body_statements,
                first_call.location.start_byte,
            ) {
                stmt.location().extract_indentation(source)
            } else {
                "  ".to_string()
            }
        } else {
            "  ".to_string()
        };

        // Track GParamSpec variable names to delete their declarations later
        let mut param_spec_vars = std::collections::HashSet::new();

        // Convert each g_object_class_install_property call
        for call in install_calls {
            // Extract the property enum value (2nd argument)
            let Some(prop_id_arg) = call.get_arg(1) else {
                continue;
            };
            let Some(prop_id) = prop_id_arg.to_source_string(source) else {
                continue;
            };

            // Extract the g_param_spec call (3rd argument)
            let Some(param_spec_arg) = call.get_arg(2) else {
                continue;
            };

            // Check if this is a variable pattern or direct call
            let (param_spec, delete_install_call) = if let Expression::Call(param_spec_call) =
                param_spec_arg
            {
                // Direct call pattern: g_object_class_install_property(...,
                // g_param_spec_xxx(...))
                let func_name = param_spec_call.function_name();
                let new_line_prefix = format!("{}[{}] = {} (", array_name, prop_id, func_name);
                let target_column = indentation.len() + new_line_prefix.len();

                let Some(param_spec_text) = param_spec_arg.to_source_string(source) else {
                    continue;
                };
                (
                    self.reindent_multiline(&param_spec_text, target_column),
                    false,
                )
            } else {
                // Variable pattern: param_spec = g_param_spec_xxx(...);
                // g_object_class_install_property(..., param_spec);
                let Some(var_name) = param_spec_arg.to_source_string(source) else {
                    continue;
                };

                // Find the assignment that comes before this install_property call
                let assignment = param_spec_assignments
                    .iter()
                    .filter(|(name, _, stmt_loc, _)| {
                        name == &var_name && stmt_loc.start_byte < call.location.start_byte
                    })
                    .max_by_key(|(_, _, stmt_loc, _)| stmt_loc.start_byte);

                if let Some((_, _property_name, statement_location, g_param_spec_call)) = assignment
                {
                    param_spec_vars.insert(var_name.clone());

                    // Use the g_param_spec call from the assignment
                    let func_name = g_param_spec_call.function_name();
                    let new_line_prefix = format!("{}[{}] = {} (", array_name, prop_id, func_name);
                    // Note: indentation is not included because it stays in place during
                    // replacement
                    let assignment_indent = statement_location.extract_indentation(source);
                    let target_column = assignment_indent.len() + new_line_prefix.len();

                    let Some(param_spec_text) =
                        Expression::Call(g_param_spec_call.clone()).to_source_string(source)
                    else {
                        continue;
                    };

                    // Replace the assignment statement with props[PROP_X] = ...
                    let replacement = format!(
                        "{}[{}] = {};",
                        array_name,
                        prop_id,
                        self.reindent_multiline(&param_spec_text, target_column)
                    );
                    fixes.push(Fix::new(
                        statement_location.start_byte,
                        statement_location.end_byte,
                        replacement,
                    ));

                    (String::new(), true) // Mark to delete install_property call
                } else {
                    // Fallback - just use the variable name as-is
                    let Some(param_spec_text) = param_spec_arg.to_source_string(source) else {
                        continue;
                    };
                    (param_spec_text, false)
                }
            };

            // Find the statement containing this install_property call
            let Some(stmt) = self.find_statement_containing_call(
                &class_init.body_statements,
                call.location.start_byte,
            ) else {
                continue;
            };

            if delete_install_call {
                // Delete the entire install_property call statement
                fixes.push(Fix::delete_line(stmt.location(), source));
            } else {
                // Replace the statement with array assignment
                let replacement = format!("{}[{}] = {};", array_name, prop_id, param_spec);
                fixes.push(Fix::new(
                    stmt.location().start_byte,
                    stmt.location().end_byte,
                    replacement,
                ));
            }
        }

        // Remove GParamSpec variable declarations
        for var_name in param_spec_vars {
            if let Some(decl_stmt) =
                self.find_param_spec_declaration(&class_init.body_statements, &var_name)
            {
                fixes.push(Fix::delete_line(decl_stmt.location(), source));
            }
        }

        // Add g_object_class_install_properties call after all assignments
        if let Some(last_call) = install_calls.last() {
            let Some(last_stmt) = self.find_statement_containing_call(
                &class_init.body_statements,
                last_call.location.start_byte,
            ) else {
                return fixes;
            };

            let install_properties_call = format!(
                "\n\n{}g_object_class_install_properties ({}, {}, {});",
                indentation, object_class_var, n_props_name, array_name
            );
            fixes.push(Fix::new(
                last_stmt.location().end_byte,
                last_stmt.location().end_byte,
                install_properties_call,
            ));
        }

        fixes
    }

    /// Determine the N_PROPS sentinel name based on enum naming convention
    fn determine_n_props_name(&self, property_enum: &gobject_ast::EnumInfo) -> String {
        // Look for common prefixes in enum values
        if let Some(first_value) = property_enum.values.first() {
            let name = &first_value.name;

            // Check for common patterns like PROP_0, WIDGET_PROP_0, etc.
            if let Some(prefix_end) = name.rfind("PROP_") {
                let prefix = &name[..prefix_end];
                if prefix.is_empty() {
                    return "N_PROPS".to_string();
                } else {
                    return format!("{}N_PROPS", prefix);
                }
            }
        }

        "N_PROPS".to_string()
    }

    /// Determine the array name, preferring "props" but using "obj_props" if
    /// "props" exists
    fn determine_array_name(&self, file: &gobject_ast::FileModel, _source: &[u8]) -> String {
        use gobject_ast::{Statement, top_level::TopLevelItem};

        // Check if "props" is already used as a GParamSpec array
        for item in &file.top_level_items {
            if let TopLevelItem::Declaration(Statement::Declaration(decl)) = item
                && decl.name == "props"
                && decl.type_info.full_text.contains("GParamSpec")
            {
                return "obj_props".to_string();
            }
        }

        "props".to_string()
    }

    fn find_statement_containing_call<'a>(
        &self,
        statements: &'a [Statement],
        call_start_byte: usize,
    ) -> Option<&'a Statement> {
        for stmt in statements {
            let loc = stmt.location();
            if call_start_byte >= loc.start_byte && call_start_byte < loc.end_byte {
                return Some(stmt);
            }
        }
        None
    }

    /// Find the GObjectClass variable name
    fn find_object_class_variable(
        &self,
        class_init: &gobject_ast::top_level::FunctionDefItem,
    ) -> Option<String> {
        use gobject_ast::Statement;

        for stmt in &class_init.body_statements {
            if let Statement::Declaration(decl) = stmt
                && decl.type_info.base_type == "GObjectClass"
            {
                return Some(decl.name.clone());
            }
        }
        None
    }

    /// Re-indent multiline text to align continuation lines to a specific
    /// column
    fn reindent_multiline(&self, text: &str, target_column: usize) -> String {
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() <= 1 {
            return text.to_string();
        }

        let continuation_indent = " ".repeat(target_column);

        let mut result = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i == 0 {
                result.push_str(line);
            } else {
                result.push('\n');
                result.push_str(&continuation_indent);
                result.push_str(line.trim_start());
            }
        }

        result
    }

    /// Find the GParamSpec variable declaration in the function body
    fn find_param_spec_declaration<'a>(
        &self,
        statements: &'a [Statement],
        var_name: &str,
    ) -> Option<&'a Statement> {
        use gobject_ast::Statement;

        for stmt in statements {
            if let Statement::Declaration(decl) = stmt
                && decl.name == var_name
                && decl.type_info.base_type == "GParamSpec"
            {
                return Some(stmt);
            }
        }

        None
    }
}
