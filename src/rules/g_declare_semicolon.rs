use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

/// Rule that enforces semicolons after G_DECLARE_* macros
///
/// Without semicolons, tree-sitter misparses the following declarations,
/// causing them to be missed by the AST parser.
pub struct GDeclareSemicolon;

impl Rule for GDeclareSemicolon {
    fn name(&self) -> &'static str {
        "g_declare_semicolon"
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
            let source = std::str::from_utf8(&file.source).unwrap_or("");

            // Track byte offset as we go through lines
            let mut byte_offset = 0;

            // Track if we're inside a G_DECLARE macro (for multi-line cases)
            let mut in_g_declare: Option<usize> = None; // Store the starting line number

            // Look for G_DECLARE_* macros
            for (line_num, line) in source.lines().enumerate() {
                let trimmed = line.trim();

                // Check if we're starting a G_DECLARE macro
                if trimmed.contains("G_DECLARE_FINAL_TYPE")
                    || trimmed.contains("G_DECLARE_DERIVABLE_TYPE")
                    || trimmed.contains("G_DECLARE_INTERFACE")
                {
                    in_g_declare = Some(line_num);
                }

                // If we're in a G_DECLARE macro, check for the closing paren
                if in_g_declare.is_some() && trimmed.contains(')') {
                    // Check if this looks like the closing line (not a nested paren)
                    // Simple heuristic: if the line ends with ) or has ) followed by
                    // whitespace/comment
                    if let Some(paren_pos) = line.rfind(')') {
                        let after_paren = line[paren_pos + 1..].trim();

                        // Check if there's no semicolon after the closing paren
                        if !after_paren.starts_with(';') {
                            // Calculate byte position right after the closing paren
                            let fix_byte_pos = byte_offset + paren_pos + 1;

                            let mut v = self.violation_with_fix(
                                path,
                                line_num + 1,
                                paren_pos + 1,
                                "G_DECLARE_* macro should end with a semicolon. Without it, tree-sitter may misparse following declarations.".to_string(),
                                Fix::new(fix_byte_pos, fix_byte_pos, ";"),
                            );
                            v.snippet = Some(format!("{}; // Add semicolon here", trimmed));
                            violations.push(v);
                        }

                        // Reset state - we've processed this G_DECLARE
                        in_g_declare = None;
                    }
                }

                // Update byte offset for next line (line length + newline)
                byte_offset += line.len() + 1;
            }
        }
    }
}
