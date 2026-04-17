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

    fn category(&self) -> super::Category {
        super::Category::Suspicious
    }

    fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<super::Violation>,
    ) {
        // Find all declared but not defined functions
        for (path, func) in self.find_declared_but_not_defined(ast_context) {
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
                func.location.line,
                1,
                format!(
                    "Function '{}' is declared in a header but has no implementation",
                    func.name
                ),
            ));
        }
    }
}

impl MissingImplementation {
    /// Find functions declared in headers that have no implementation
    /// Returns (file_path, function_decl) tuples
    pub fn find_declared_but_not_defined<'a>(
        &self,
        ast_context: &'a AstContext,
    ) -> Vec<(
        &'a std::path::Path,
        &'a gobject_ast::top_level::FunctionDeclItem,
    )> {
        ast_context
            .project
            .files
            .iter()
            .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "h"))
            .flat_map(|(path, file)| {
                file.iter_function_declarations()
                    .filter(|f| {
                        // Check if there's a matching definition in any C file
                        !ast_context
                            .project
                            .files
                            .iter()
                            .filter(|(p, _)| p.extension().is_some_and(|ext| ext == "c"))
                            .any(|(_, c_file)| {
                                c_file
                                    .iter_function_definitions()
                                    .any(|def| def.name == f.name)
                            })
                    })
                    .map(move |f| (path.as_path(), f))
            })
            .collect()
    }
}
