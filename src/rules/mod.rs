use crate::ast_context::AstContext;
use crate::config::Config;
use std::path::PathBuf;

pub mod chainup;
pub mod deprecated_add_private;
pub mod g_param_spec;
pub mod gdeclare_semicolon;
pub mod gerror_init;
pub mod gtask_source_tag;
pub mod missing_implementation;
pub mod property_enum_zero;
pub mod strcmp_equal;
pub mod suggest_g_autoptr_goto;
pub mod unnecessary_null_check;
pub mod use_clear_functions;
pub mod use_g_clear_error;
pub mod use_g_set_str;
pub mod use_g_strcmp0;

#[derive(Debug, Clone)]
pub struct Violation {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub rule: &'static str,
    pub snippet: Option<String>,
    /// Rule execution order - higher means more specific/later rules take precedence
    pub rule_index: usize,
}

/// Trait that all linting rules must implement
pub trait Rule {
    /// The unique identifier for this rule (e.g., "missing_implementation")
    fn name(&self) -> &'static str;

    /// Check the AST and add violations to the provided vector
    fn check_all(&self, ast_context: &AstContext, config: &Config, violations: &mut Vec<Violation>);

    /// Helper to create a violation with the rule name automatically filled in
    fn violation(
        &self,
        file: &std::path::Path,
        line: usize,
        column: usize,
        message: String,
    ) -> Violation {
        Violation {
            file: file.to_path_buf(),
            line,
            column,
            message,
            rule: self.name(),
            snippet: None,
            rule_index: 0, // Will be set by scanner based on execution order
        }
    }
}
