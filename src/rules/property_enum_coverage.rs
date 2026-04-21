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
                let (class_init, assignments) =
                    match file.find_class_init_for_property_enum(enum_info) {
                        Some(pair) => pair,
                        None => continue,
                    };

                // Collect all installed property enum values
                let installed_properties: Vec<_> = assignments
                    .into_iter()
                    .filter_map(|assignment| assignment.get_installed_enum_value(&file.source))
                    .collect();

                // Check coverage
                for prop_name in property_values {
                    if !installed_properties.iter().any(|p| p == prop_name) {
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
