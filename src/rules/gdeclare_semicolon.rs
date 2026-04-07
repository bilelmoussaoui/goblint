use super::Violation;
use crate::ast_context::AstContext;
use crate::config::Config;

/// Rule that enforces semicolons after G_DECLARE_* macros
///
/// Without semicolons, tree-sitter misparses the following declarations,
/// causing them to be missed by the AST parser.
pub struct GDeclareSemicolon;

impl GDeclareSemicolon {
    pub fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        for (path, file) in ast_context.iter_header_files() {
            // Use the already-loaded source from the file model
            let source = String::from_utf8_lossy(&file.source);

            // Look for G_DECLARE_* macros
            for (line_num, line) in source.lines().enumerate() {
                let trimmed = line.trim();

                // Check if this line contains a G_DECLARE macro call end
                if trimmed.contains("G_DECLARE_FINAL_TYPE")
                    || trimmed.contains("G_DECLARE_DERIVABLE_TYPE")
                    || trimmed.contains("G_DECLARE_INTERFACE")
                {
                    // Check if it's the closing line (contains closing paren)
                    if trimmed.contains(')') && !trimmed.ends_with(");") {
                        // Find the closing paren and check what comes after
                        if let Some(paren_pos) = trimmed.rfind(')') {
                            let after_paren = &trimmed[paren_pos + 1..].trim();
                            if after_paren.is_empty() {
                                violations.push(Violation {
                                    file: path.to_owned(),
                                    line: line_num + 1,
                                    column: paren_pos + 1,
                                    message: "G_DECLARE_* macro should end with a semicolon. Without it, tree-sitter may misparse following declarations.".to_string(),
                                    rule: "gdeclare_semicolon",
                                    snippet: Some(format!("{}; // Add semicolon here", trimmed)),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}
