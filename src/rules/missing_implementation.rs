use super::Violation;
use crate::ast_context::AstContext;
use crate::config::Config;

/// Rule that checks for functions declared in headers but never implemented
pub struct MissingImplementation;

impl MissingImplementation {
    pub fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        // Find all declared but not defined functions
        for (path, func) in ast_context.find_declared_but_not_defined() {
            // Skip static function declarations - they're file-local and often forward declarations
            // within the same header file
            if func.is_static {
                continue;
            }

            // Skip functions ending with _quark - these are often macro-generated
            if func.name.ends_with("_quark") {
                continue;
            }

            violations.push(Violation {
                file: path.display().to_string(),
                line: func.line,
                column: 1,
                message: format!(
                    "Function '{}' is declared in a header but has no implementation",
                    func.name
                ),
                rule: "missing_implementation".to_string(),
                snippet: None,
            });
        }
    }
}
