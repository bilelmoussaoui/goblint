use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

/// Rule that enforces semicolons after G_DECLARE_* macros
///
/// Without semicolons, tree-sitter misparses the following declarations,
/// causing them to be missed by the AST parser.
pub struct GDeclareSemicolon;

impl Rule for GDeclareSemicolon {
    fn name(&self) -> &'static str {
        "gdeclare_semicolon"
    }

    fn description(&self) -> &'static str {
        "Enforce semicolons after G_DECLARE_* macros"
    }

    fn category(&self) -> super::Category {
        super::Category::Pedantic
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
        for (path, file) in ast_context.iter_header_files() {
            // Use the already-loaded source from the file model
            let source = String::from_utf8_lossy(&file.source);

            // Track byte offset as we go through lines
            let mut byte_offset = 0;

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
                        if let Some(paren_pos) = line.rfind(')') {
                            let after_paren = &line[paren_pos + 1..].trim();
                            if after_paren.is_empty() {
                                // Calculate byte position right after the closing paren
                                let fix_byte_pos = byte_offset + paren_pos + 1;

                                let mut v = self.violation_with_fix(
                                    path,
                                    line_num + 1,
                                    paren_pos + 1,
                                    "G_DECLARE_* macro should end with a semicolon. Without it, tree-sitter may misparse following declarations.".to_string(),
                                    Fix {
                                        start_byte: fix_byte_pos,
                                        end_byte: fix_byte_pos,
                                        replacement: ";".to_string(),
                                    },
                                );
                                v.snippet = Some(format!("{}; // Add semicolon here", trimmed));
                                violations.push(v);
                            }
                        }
                    }
                }

                // Update byte offset for next line (line length + newline)
                byte_offset += line.len() + 1;
            }
        }
    }
}
