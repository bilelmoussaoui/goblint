use gobject_ast::{Expression, Statement};

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UnnecessaryNullCheck;

impl Rule for UnnecessaryNullCheck {
    fn name(&self) -> &'static str {
        "unnecessary_null_check"
    }

    fn description(&self) -> &'static str {
        "Detect unnecessary NULL checks before g_free/g_clear_* functions"
    }

    fn category(&self) -> super::Category {
        super::Category::Suspicious
    }

    fn fixable(&self) -> bool {
        true
    }

    fn check_func_impl(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        func: &gobject_ast::FunctionInfo,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        if !func.is_definition {
            return;
        }

        let source = &ast_context.project.files.get(path).unwrap().source;
        // Walk through function body looking for if statements
        self.check_statements(&func.body_statements, path, source, violations);
    }
}

impl UnnecessaryNullCheck {
    fn check_statements(
        &self,
        statements: &[Statement],
        file_path: &std::path::Path,
        source: &[u8],
        violations: &mut Vec<Violation>,
    ) {
        for stmt in statements {
            match stmt {
                Statement::If(if_stmt) => {
                    self.check_if_statement(if_stmt, file_path, source, violations);
                    // Recursively check nested statements
                    self.check_statements(&if_stmt.then_body, file_path, source, violations);
                    if let Some(else_body) = &if_stmt.else_body {
                        self.check_statements(else_body, file_path, source, violations);
                    }
                }
                Statement::Compound(compound) => {
                    self.check_statements(&compound.statements, file_path, source, violations);
                }
                Statement::Labeled(labeled) => {
                    self.check_statements(
                        std::slice::from_ref(&labeled.statement),
                        file_path,
                        source,
                        violations,
                    );
                }
                _ => {}
            }
        }
    }

    fn check_if_statement(
        &self,
        if_stmt: &gobject_ast::IfStatement,
        file_path: &std::path::Path,
        source: &[u8],
        violations: &mut Vec<Violation>,
    ) {
        // Don't flag if there's an else branch — removing the if would also drop the
        // else logic
        if if_stmt.has_else() {
            return;
        }

        // Extract variable being checked (e.g., "ptr" from "ptr != NULL")
        let Some(checked_var) = if_stmt.extract_null_check_variable() else {
            return;
        };

        // Check if the body contains only a g_free/g_clear_* call with the checked
        // variable
        if !if_stmt.has_single_statement() {
            return;
        }

        // Get the single statement in the then body
        let Statement::Expression(expr_stmt) = &if_stmt.then_body[0] else {
            return;
        };

        // Check if it's a g_free/g_clear_* call
        let Expression::Call(call) = &expr_stmt.expr else {
            return;
        };

        // Check for g_free or any g_clear_* function
        if !call.function.starts_with("g_free") && !call.function.starts_with("g_clear_") {
            return;
        }

        // Check if the call arguments reference the checked variable
        let mut references_var = false;
        for arg in &call.arguments {
            // Check if the argument references our variable
            let gobject_ast::Argument::Expression(arg_expr) = arg;
            arg_expr.walk(&mut |e| {
                if let Expression::Identifier(id) = e
                    && id.name == checked_var
                {
                    references_var = true;
                }
            });
        }

        if !references_var {
            return;
        }

        // Create a fix: replace the if statement with the call statement
        // Extract the statement text from the source
        let stmt_text = std::str::from_utf8(
            &source[expr_stmt.location.start_byte..expr_stmt.location.end_byte],
        )
        .unwrap_or("");

        let fix = Fix::new(
            if_stmt.location.start_byte,
            if_stmt.location.end_byte,
            stmt_text.to_string(),
        );

        violations.push(self.violation_with_fix(
            file_path,
            if_stmt.location.line,
            if_stmt.location.column,
            format!(
                "Remove unnecessary NULL check before {} ({} handles NULL)",
                call.function, call.function
            ),
            fix,
        ));
    }
}
