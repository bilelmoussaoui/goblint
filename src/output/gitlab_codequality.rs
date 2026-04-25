use crate::rules::Violation;
use serde_json::json;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

pub fn generate_gitlab_codequality(violations: &[Violation], project_root: &Path) -> String {
    let issues = violations
        .iter()
        .map(|v| generate_issue(v, project_root))
        .collect::<Vec<_>>();

    serde_json::to_string_pretty(&issues).unwrap()
}

/// Generate a unique fingerprint to identify a violation.
fn geneerate_issue_fingerprint(violation: &Violation, relative_path: &str) -> String {
    let mut hasher = DefaultHasher::new();

    violation.rule.hash(&mut hasher);
    relative_path.hash(&mut hasher);
    violation.line.hash(&mut hasher);
    violation.column.hash(&mut hasher);

    hasher.finish().to_string()
}

/// Generate a Gitlab CodeQuality issue report
/// The format is very similar to CodeClimate reports
/// https://docs.gitlab.com/ci/testing/code_quality/#code-quality-report-format
fn generate_issue(violation: &Violation, project_root: &Path) -> serde_json::Value {
    // Make file path relative to project root for Gitlab CodeQuality
    let relative_path = violation
        .file
        .strip_prefix(project_root)
        .unwrap_or(&violation.file)
        .display()
        .to_string();

    let location = json!({
        "path": relative_path,
        "positions": {
            "begin": {
                "line": violation.line,
                "column": violation.column
            }
        },
    });

    json!({
        "description": violation.message,
        "check_name": violation.rule,
        "fingerprint": geneerate_issue_fingerprint(violation, &relative_path),
        "severity": rule_level_to_codequality_severity(violation.level),
        "categories": [violation.category],
        "location": location,
    })
}

// The severity of the violation, can be one of info, minor, major, critical, or blocker.
fn rule_level_to_codequality_severity(level: crate::config::RuleLevel) -> &'static str {
    match level {
        crate::config::RuleLevel::Error => "blocker",
        // FIXME: Decide which one?
        // crate::config::RuleLevel::Warn => "critical",
        crate::config::RuleLevel::Warn => "info",
        crate::config::RuleLevel::Ignore => {
            unreachable!("Ignored violations should not be reported")
        }
    }
}
