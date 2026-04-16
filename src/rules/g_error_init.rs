use gobject_ast::{Expression, Statement};

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
        func: &gobject_ast::FunctionInfo,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        if !func.is_definition {
            return;
        }

        // Walk all statements and check declarations
        for stmt in &func.body_statements {
            stmt.walk(&mut |s| {
                self.check_statement(path, s, violations);
            });
        }
    }
}

impl GErrorInit {
    fn check_statement(
        &self,
        file_path: &std::path::Path,
        stmt: &Statement,
        violations: &mut Vec<Violation>,
    ) {
        let Statement::Declaration(decl) = stmt else {
            return;
        };

        // Check if this is a GError* declaration
        if !decl.type_name.contains("GError") || !decl.type_name.contains('*') {
            return;
        }

        // Check if it's initialized to NULL
        let is_initialized_to_null = match &decl.initializer {
            None => false,
            Some(expr) if expr.is_null() || expr.is_zero() => true,
            Some(Expression::Identifier(i)) if i.name == "NULL" => true,
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
