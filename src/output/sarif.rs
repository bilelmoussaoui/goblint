use std::path::Path;

use serde_json::json;

use crate::{config::Config, rules::Violation};

/// Generate SARIF 2.1.0 output for violations
pub fn generate_sarif(violations: &[Violation], config: &Config, project_root: &Path) -> String {
    let rules = generate_rules_metadata(config);
    let results = violations
        .iter()
        .map(|v| generate_result(v, config, project_root))
        .collect::<Vec<_>>();

    let sarif = json!({
        "version": "2.1.0",
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "gobject-lint",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/bilelmoussaoui/gobject-lint",
                    "rules": rules,
                }
            },
            "results": results,
        }]
    });

    serde_json::to_string_pretty(&sarif).unwrap()
}

/// Generate rule metadata for enabled rules only
fn generate_rules_metadata(config: &Config) -> Vec<serde_json::Value> {
    use crate::scanner::create_all_rules;

    let rules = create_all_rules(config);
    rules
        .iter()
        .filter(|entry| entry.level.is_enabled())
        .map(|entry| {
            json!({
                "id": entry.rule.name(),
                "shortDescription": {
                    "text": entry.rule.description(),
                },
                "fullDescription": {
                    "text": entry.rule.description(),
                },
                "defaultConfiguration": {
                    "level": rule_level_to_sarif_level(entry.level),
                },
                "properties": {
                    "category": entry.rule.category().as_str(),
                    "tags": [entry.rule.category().as_str()],
                },
            })
        })
        .collect()
}

/// Generate a SARIF result from a violation
fn generate_result(
    violation: &Violation,
    config: &Config,
    project_root: &Path,
) -> serde_json::Value {
    // Make file path relative to project root for GitHub Code Scanning
    let relative_path = violation
        .file
        .strip_prefix(project_root)
        .unwrap_or(&violation.file)
        .display()
        .to_string();

    let mut result = json!({
        "ruleId": violation.rule,
        "level": rule_level_to_sarif_level(violation.level),
        "message": {
            "text": violation.message,
        },
        "locations": [{
            "physicalLocation": {
                "artifactLocation": {
                    "uri": relative_path,
                },
                "region": {
                    "startLine": violation.line,
                    "startColumn": violation.column,
                }
            }
        }]
    });

    // Add fix if available
    if let Some(ref fix) = violation.fix {
        // Convert byte offsets to line/column positions for SARIF
        if let Ok(content) = std::fs::read(&violation.file)
            && let (Some(start_pos), Some(end_pos)) = (
                byte_offset_to_position(&content, fix.start_byte),
                byte_offset_to_position(&content, fix.end_byte),
            )
        {
            let deleted_region = json!({
                "startLine": start_pos.0,
                "startColumn": start_pos.1,
                "endLine": end_pos.0,
                "endColumn": end_pos.1,
            });

            result["fixes"] = json!([{
                "description": {
                    "text": format!("Apply {}", violation.rule),
                },
                "artifactChanges": [{
                    "artifactLocation": {
                        "uri": relative_path,
                    },
                    "replacements": [{
                        "deletedRegion": deleted_region,
                        "insertedContent": {
                            "text": fix.replacement,
                        }
                    }]
                }]
            }]);
        }
    }

    // Add editor URL if configured
    if let Some(ref editor_url) = config.editor_url {
        let url = editor_url
            .replace("{path}", &violation.file.display().to_string())
            .replace("{line}", &violation.line.to_string())
            .replace("{column}", &violation.column.to_string());

        result["hovers"] = json!([{
            "text": format!("Open in editor: {}", url),
        }]);
    }

    result
}

/// Map rule level to SARIF level
fn rule_level_to_sarif_level(level: crate::config::RuleLevel) -> &'static str {
    match level {
        crate::config::RuleLevel::Error => "error",
        crate::config::RuleLevel::Warn => "warning",
        crate::config::RuleLevel::Ignore => "none", // Should never happen
    }
}

/// Convert byte offset to (line, column) position
/// Returns None if offset is out of bounds
fn byte_offset_to_position(content: &[u8], offset: usize) -> Option<(usize, usize)> {
    if offset > content.len() {
        return None;
    }

    let mut line = 1;
    let mut column = 1;

    for (i, &byte) in content.iter().enumerate() {
        if i == offset {
            return Some((line, column));
        }

        if byte == b'\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    // Handle case where offset is exactly at the end
    if offset == content.len() {
        Some((line, column))
    } else {
        None
    }
}
