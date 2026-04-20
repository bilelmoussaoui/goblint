use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

/// Rule that enforces semicolons after G_DECLARE_* and G_DEFINE_* macros
///
/// Without semicolons, tree-sitter misparses the following declarations,
/// causing them to be missed by the AST parser.
pub struct GDeclareSemicolon;

impl Rule for GDeclareSemicolon {
    fn name(&self) -> &'static str {
        "g_declare_semicolon"
    }

    fn description(&self) -> &'static str {
        "Enforce semicolons after G_DECLARE_* and G_DEFINE_* macros"
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
        // Check both header files and C files
        for (path, file) in ast_context.iter_all_files() {
            let source = &file.source;

            // Use the AST to find all G_DECLARE_* and G_DEFINE_* macros
            for gobject_type in file.iter_all_gobject_types() {
                let macro_name = gobject_type.kind.macro_name();
                let location = &gobject_type.location;

                // Check if there's a semicolon after the macro (location.end_byte points right
                // after the closing paren)
                let mut check_pos = location.end_byte;
                let mut has_semicolon = false;

                while check_pos < source.len() {
                    let ch = source[check_pos];
                    if ch == b';' {
                        has_semicolon = true;
                        break;
                    } else if ch == b' ' || ch == b'\t' || ch == b'\n' || ch == b'\r' {
                        check_pos += 1;
                    } else {
                        break;
                    }
                }

                if !has_semicolon {
                    // Calculate line and column for end_byte
                    let (end_line, end_column) =
                        self.calculate_line_column(source, location.end_byte);

                    let mut v = self.violation_with_fix(
                        path,
                        end_line,
                        end_column,
                        format!(
                            "{} macro should end with a semicolon. Without it, tree-sitter may misparse following declarations.",
                            macro_name
                        ),
                        Fix::new(location.end_byte, location.end_byte, ";"),
                    );

                    // Extract snippet from source
                    if let Ok(source_str) = std::str::from_utf8(source) {
                        let snippet = source_str.lines().nth(end_line - 1).unwrap_or("").trim();
                        v.snippet = Some(format!("{}; // Add semicolon here", snippet));
                    }

                    violations.push(v);
                }
            }
        }
    }
}

impl GDeclareSemicolon {
    /// Calculate line and column number from byte offset
    fn calculate_line_column(&self, source: &[u8], byte_offset: usize) -> (usize, usize) {
        let mut line = 1;
        let mut column = 1;

        for &byte in &source[..byte_offset.min(source.len())] {
            if byte == b'\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }

        (line, column)
    }
}
