use std::collections::HashSet;

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct PropertyEnumZero;

impl Rule for PropertyEnumZero {
    fn name(&self) -> &'static str {
        "property_enum_convention"
    }

    fn description(&self) -> &'static str {
        "Ensure property enums start with PROP_0 and end with N_PROPS"
    }

    fn category(&self) -> super::Category {
        super::Category::Correctness
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
        // Check each file's enums
        for (path, file) in ast_context.iter_all_files() {
            // First pass: collect all existing PROP_0 variants
            let existing_prop_zeros: HashSet<String> = file
                .iter_all_enums()
                .flat_map(|enum_info| &enum_info.values)
                .filter_map(|val| {
                    if val.name.ends_with("_PROP_0") || val.name == "PROP_0" {
                        Some(val.name.clone())
                    } else {
                        None
                    }
                })
                .collect();

            let mut will_add_unprefixed_prop_zero = existing_prop_zeros.contains("PROP_0");

            // Second pass: check each enum
            for enum_info in file.iter_all_enums() {
                if !self.is_property_enum(enum_info) {
                    continue;
                }

                // Determine if we need a prefix
                let prefix = if will_add_unprefixed_prop_zero {
                    Some(self.to_screaming_snake_case(&enum_info.name) + "_")
                } else {
                    None
                };

                let mut fixes = Vec::new();
                let mut has_violations = false;
                let mut violation_line = enum_info.location.line;
                let mut message = String::new();

                // Check first enumerator
                if let Some(first_prop_idx) = self.find_first_prop_index(enum_info) {
                    let first_val = &enum_info.values[first_prop_idx];

                    // Check if it's PROP_0 or XXX_PROP_0
                    if !(first_val.name.ends_with("_PROP_0") || first_val.name == "PROP_0") {
                        has_violations = true;
                        violation_line = enum_info.location.line;

                        // Get indentation from the source
                        let indent = self.get_indentation(&file.source, first_val.name_start_byte);

                        let prop_zero_name = if let Some(ref p) = prefix {
                            format!("{}PROP_0", p)
                        } else {
                            "PROP_0".to_string()
                        };

                        // Insert PROP_0 before first property
                        let insertion = format!("{},\n{}", prop_zero_name, indent);
                        fixes.push(Fix::new(
                            first_val.start_byte,
                            first_val.start_byte,
                            insertion,
                        ));

                        // Remove " = 0" if it exists
                        if first_val.value == Some(0)
                            && let (Some(_value_start), Some(value_end)) =
                                (first_val.value_start_byte, first_val.value_end_byte)
                        {
                            fixes.push(Fix::new(first_val.name_end_byte, value_end, ""));
                        }

                        message = format!(
                            "Property enum should start with {}, not {}",
                            prop_zero_name, first_val.name
                        );

                        // Mark that we're adding PROP_0
                        if prefix.is_none() {
                            will_add_unprefixed_prop_zero = true;
                        }
                    }
                }

                // Check last enumerator
                if let Some(last) = enum_info.values.last()
                    && !last.name.ends_with("_N_PROPS")
                    && last.name != "N_PROPS"
                    && !last.name.ends_with("_PROP_LAST")
                    && last.name != "PROP_LAST"
                {
                    has_violations = true;

                    let indent = self.get_indentation(&file.source, last.name_start_byte);

                    let n_props_name = if let Some(ref p) = prefix {
                        format!("{}N_PROPS", p)
                    } else {
                        "N_PROPS".to_string()
                    };

                    // Insert N_PROPS after last enumerator
                    let insertion = format!(",\n{}{}", indent, n_props_name);
                    fixes.push(Fix::new(last.end_byte, last.end_byte, insertion));

                    if !message.is_empty() {
                        message.push_str(&format!(", and should end with {}", n_props_name));
                    } else {
                        message = format!("Property enum should end with {}", n_props_name);
                    }
                }

                if has_violations {
                    violations.push(self.violation_with_fixes(
                        path,
                        violation_line,
                        1,
                        message,
                        fixes,
                    ));
                }
            }
        }
    }
}

impl PropertyEnumZero {
    fn is_property_enum(&self, enum_info: &gobject_ast::EnumInfo) -> bool {
        enum_info
            .values
            .iter()
            .any(|v| v.name.contains("_PROP_") || v.name.starts_with("PROP_"))
    }

    fn find_first_prop_index(&self, enum_info: &gobject_ast::EnumInfo) -> Option<usize> {
        enum_info
            .values
            .iter()
            .position(|v| v.name.contains("_PROP_") || v.name.starts_with("PROP_"))
    }

    fn to_screaming_snake_case(&self, name: &str) -> String {
        let mut result = String::new();
        let mut prev_was_lowercase = false;

        for ch in name.chars() {
            if ch.is_uppercase() && prev_was_lowercase {
                result.push('_');
            }
            result.push(ch.to_ascii_uppercase());
            prev_was_lowercase = ch.is_lowercase();
        }

        result
    }

    fn get_indentation(&self, source: &[u8], name_start_byte: usize) -> String {
        let mut indent_start = name_start_byte;
        while indent_start > 0 && source[indent_start - 1] != b'\n' {
            indent_start -= 1;
        }
        std::str::from_utf8(&source[indent_start..name_start_byte])
            .unwrap_or("  ")
            .to_string()
    }
}
