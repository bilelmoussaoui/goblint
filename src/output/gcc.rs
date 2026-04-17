use std::env;

use crate::rules::Violation;

/// Generate GCC-compatible output format
/// Paths are relative to CWD (not project root) to match GCC/ninja behavior
pub fn generate_gcc(violations: &[Violation]) {
    // Count errors and warnings
    let error_count = violations.iter().filter(|v| v.level.is_error()).count();
    let warning_count = violations.iter().filter(|v| v.level.is_warn()).count();

    // Print summary at the top
    if error_count > 0 && warning_count > 0 {
        eprintln!(
            "Found {} error(s) and {} warning(s)",
            error_count, warning_count
        );
    } else if error_count > 0 {
        eprintln!("Found {} error(s)", error_count);
    } else if warning_count > 0 {
        eprintln!("Found {} warning(s)", warning_count);
    } else {
        eprintln!("No violations found");
    }

    // Get current working directory for relative path calculation
    let cwd = env::current_dir().ok();

    for violation in violations {
        // Make path relative to CWD if possible
        let display_path = if let Some(ref cwd) = cwd {
            violation
                .file
                .strip_prefix(cwd)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| violation.file.display().to_string())
        } else {
            violation.file.display().to_string()
        };

        let level = match violation.level {
            crate::config::RuleLevel::Error => "error",
            crate::config::RuleLevel::Warn => "warning",
            crate::config::RuleLevel::Ignore => {
                unreachable!("Ignored violations should not be reported")
            }
        };

        println!(
            "{}:{}:{}: {}: {} [{}]",
            display_path,
            violation.line,
            violation.column,
            level,
            violation.message,
            violation.rule
        );
    }
}
