use std::collections::HashMap;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct MatchingDeclareDefine;

impl Rule for MatchingDeclareDefine {
    fn name(&self) -> &'static str {
        "matching_declare_define"
    }

    fn description(&self) -> &'static str {
        "Ensure G_DECLARE_* and G_DEFINE_* macros are used consistently"
    }

    fn category(&self) -> super::Category {
        super::Category::Pedantic
    }

    fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        // Build a map of type_name -> declaration_type from all files
        let mut declared_types: HashMap<String, String> = HashMap::new();

        // Scan all files for G_DECLARE_* macros (can be in headers or C files)
        for (_path, file) in ast_context.iter_all_files() {
            self.collect_declare_macros(&file.source, &mut declared_types);
        }

        // Scan C files for mismatched G_DEFINE_* macros
        for (path, file) in ast_context.iter_c_files() {
            self.check_define_macros(&file.source, path, &declared_types, violations);
        }
    }
}

impl MatchingDeclareDefine {
    fn collect_declare_macros(&self, source: &[u8], declared_types: &mut HashMap<String, String>) {
        let source_str = String::from_utf8_lossy(source);

        for line in source_str.lines() {
            let trimmed = line.trim();

            // G_DECLARE_FINAL_TYPE (TypeName, type_name, ...)
            if trimmed.starts_with("G_DECLARE_FINAL_TYPE") {
                if let Some(type_name) = self.extract_type_name_from_declare(trimmed) {
                    declared_types.insert(type_name, "G_DECLARE_FINAL_TYPE".to_string());
                }
            }
            // G_DECLARE_DERIVABLE_TYPE (TypeName, type_name, ...)
            else if trimmed.starts_with("G_DECLARE_DERIVABLE_TYPE") {
                if let Some(type_name) = self.extract_type_name_from_declare(trimmed) {
                    declared_types.insert(type_name, "G_DECLARE_DERIVABLE_TYPE".to_string());
                }
            }
            // G_DECLARE_INTERFACE (TypeName, type_name, ...)
            else if trimmed.starts_with("G_DECLARE_INTERFACE")
                && let Some(type_name) = self.extract_type_name_from_declare(trimmed)
            {
                declared_types.insert(type_name, "G_DECLARE_INTERFACE".to_string());
            }
        }
    }

    fn extract_type_name_from_declare(&self, line: &str) -> Option<String> {
        // G_DECLARE_FINAL_TYPE (TypeName, type_name, ...)
        // Extract TypeName (first argument)
        let start = line.find('(')?;
        let rest = &line[start + 1..];
        let first_arg = rest.split(',').next()?.trim();

        // Remove trailing parenthesis if present
        let type_name = first_arg.trim_end_matches(')').trim();

        if type_name.is_empty() || !type_name.chars().next()?.is_uppercase() {
            return None;
        }

        Some(type_name.to_string())
    }

    fn check_define_macros(
        &self,
        source: &[u8],
        path: &std::path::Path,
        declared_types: &HashMap<String, String>,
        violations: &mut Vec<Violation>,
    ) {
        let source_str = String::from_utf8_lossy(source);

        for (line_num, line) in source_str.lines().enumerate() {
            let trimmed = line.trim();

            // Check all G_DEFINE_* variants
            let (define_macro, type_name) = if trimmed.starts_with("G_DEFINE_FINAL_TYPE") {
                (
                    "G_DEFINE_FINAL_TYPE",
                    self.extract_type_name_from_define(trimmed),
                )
            } else if trimmed.starts_with("G_DEFINE_DERIVABLE_TYPE") {
                (
                    "G_DEFINE_DERIVABLE_TYPE",
                    self.extract_type_name_from_define(trimmed),
                )
            } else if trimmed.starts_with("G_DEFINE_INTERFACE") {
                (
                    "G_DEFINE_INTERFACE",
                    self.extract_type_name_from_define(trimmed),
                )
            } else if trimmed.starts_with("G_DEFINE_TYPE")
                && !trimmed.starts_with("G_DEFINE_TYPE_EXTENDED")
                && !trimmed.starts_with("G_DEFINE_TYPE_WITH_PRIVATE")
                && !trimmed.starts_with("G_DEFINE_TYPE_WITH_CODE")
            {
                ("G_DEFINE_TYPE", self.extract_type_name_from_define(trimmed))
            } else {
                continue;
            };

            if let Some(type_name) = type_name {
                // Check for mismatches
                if let Some(declare_macro) = declared_types.get(&type_name) {
                    let expected_define = match declare_macro.as_str() {
                        "G_DECLARE_FINAL_TYPE" => "G_DEFINE_FINAL_TYPE",
                        "G_DECLARE_DERIVABLE_TYPE" => "G_DEFINE_DERIVABLE_TYPE",
                        "G_DECLARE_INTERFACE" => "G_DEFINE_INTERFACE",
                        _ => continue,
                    };

                    if define_macro != expected_define {
                        violations.push(self.violation(
                            path,
                            line_num + 1,
                            1,
                            format!(
                                "Use {} instead of {} for '{}' (declared with {})",
                                expected_define, define_macro, type_name, declare_macro
                            ),
                        ));
                    }
                } else {
                    // Type is defined but not declared
                    if define_macro == "G_DEFINE_FINAL_TYPE"
                        || define_macro == "G_DEFINE_DERIVABLE_TYPE"
                        || define_macro == "G_DEFINE_INTERFACE"
                    {
                        let expected_declare = match define_macro {
                            "G_DEFINE_FINAL_TYPE" => "G_DECLARE_FINAL_TYPE",
                            "G_DEFINE_DERIVABLE_TYPE" => "G_DECLARE_DERIVABLE_TYPE",
                            "G_DEFINE_INTERFACE" => "G_DECLARE_INTERFACE",
                            _ => continue,
                        };

                        violations.push(self.violation(
                            path,
                            line_num + 1,
                            1,
                            format!(
                                "Type '{}' uses {} but has no corresponding {} in header",
                                type_name, define_macro, expected_declare
                            ),
                        ));
                    }
                }
            }
        }
    }

    fn extract_type_name_from_define(&self, line: &str) -> Option<String> {
        // G_DEFINE_TYPE (TypeName, type_name, PARENT_TYPE)
        // Extract TypeName (first argument)
        let start = line.find('(')?;
        let rest = &line[start + 1..];
        let first_arg = rest.split(',').next()?.trim();

        // Remove trailing parenthesis if present
        let type_name = first_arg.trim_end_matches(')').trim();

        if type_name.is_empty() || !type_name.chars().next()?.is_uppercase() {
            return None;
        }

        Some(type_name.to_string())
    }
}
