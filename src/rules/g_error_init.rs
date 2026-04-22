use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct GErrorInit;

impl Rule for GErrorInit {
    fn name(&self) -> &'static str {
        "g_error_init"
    }

    fn description(&self) -> &'static str {
        "Ensure GError* variables are initialized to NULL"
    }

    fn category(&self) -> super::Category {
        super::Category::Correctness
    }

    fn fixable(&self) -> bool {
        true
    }

    fn check_func_impl(
        &self,
        _ast_context: &AstContext,
        _config: &Config,
        func: &gobject_ast::top_level::FunctionDefItem,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        // Walk all statements and check declarations
        for stmt in &func.body_statements {
            for decl in stmt.iter_declarations() {
                self.check_declaration(path, decl, violations);
            }
        }
    }
}

impl GErrorInit {
    fn check_declaration(
        &self,
        file_path: &std::path::Path,
        decl: &gobject_ast::VariableDecl,
        violations: &mut Vec<Violation>,
    ) {
        // Check if this is a GError* declaration
        if !decl.type_info.is_base_type("GError") || !decl.type_info.is_pointer() {
            return;
        }

        // Check if it's initialized to NULL
        let is_initialized_to_null = match &decl.initializer {
            None => false,
            Some(expr) if expr.is_null() || expr.is_zero() => true,
            // Skip it - the fix would insert `= NULL` producing invalid code
            Some(_) => return,
        };

        if is_initialized_to_null {
            return;
        }

        // Need to add = NULL before the semicolon
        // The end_byte is after the semicolon, so insert at end_byte - 1
        let insert_pos = decl.location.end_byte - 1;

        let fix = Fix::new(insert_pos, insert_pos, " = NULL".to_string());

        violations.push(self.violation_with_fix(
            file_path,
            decl.location.line,
            decl.location.column,
            format!("GError *{} must be initialized to NULL", decl.name),
            fix,
        ));
    }
}
