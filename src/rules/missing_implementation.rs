use super::Rule;
use crate::{ast_context::AstContext, config::Config};

/// Rule that checks for functions declared in headers but never implemented
pub struct MissingImplementation;

impl Rule for MissingImplementation {
    fn name(&self) -> &'static str {
        "missing_implementation"
    }

    fn description(&self) -> &'static str {
        "Report functions declared in headers but not implemented"
    }

    fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<super::Violation>,
    ) {
        // Find all declared but not defined functions
        for (path, func) in ast_context.find_declared_but_not_defined() {
            // Skip static function declarations - they're file-local and often forward
            // declarations within the same header file
            if func.is_static {
                continue;
            }

            // Skip functions ending with _quark - these are often macro-generated
            if func.name.ends_with("_quark") {
                continue;
            }

            violations.push(self.violation(
                path,
                func.line,
                1,
                format!(
                    "Function '{}' is declared in a header but has no implementation",
                    func.name
                ),
            ));
        }
    }
}
