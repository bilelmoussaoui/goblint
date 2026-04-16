use gobject_ast::{
    Assignment, AssignmentOp, CallExpression, Expression, ExpressionStmt, Statement,
};

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGBytesUnrefToData;

impl Rule for UseGBytesUnrefToData {
    fn name(&self) -> &'static str {
        "use_g_bytes_unref_to_data"
    }

    fn description(&self) -> &'static str {
        "Use g_bytes_unref_to_data() instead of g_bytes_get_data() + g_bytes_unref()"
    }

    fn category(&self) -> super::Category {
        super::Category::Complexity
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
        self.check_statement_list(path, &func.body_statements, source, violations);
    }
}

impl UseGBytesUnrefToData {
    /// Check a list of statements for consecutive g_bytes_get_data +
    /// g_bytes_unref pattern
    fn check_statement_list(
        &self,
        file_path: &std::path::Path,
        statements: &[Statement],
        source: &[u8],
        violations: &mut Vec<Violation>,
    ) {
        // Check consecutive statements at this level
        for i in 0..statements.len().saturating_sub(1) {
            self.try_bytes_pattern(
                file_path,
                &statements[i],
                &statements[i + 1],
                source,
                violations,
            );
        }

        // Recurse into nested statement blocks
        for stmt in statements {
            match stmt {
                Statement::If(if_stmt) => {
                    self.check_statement_list(file_path, &if_stmt.then_body, source, violations);
                    if let Some(else_body) = &if_stmt.else_body {
                        self.check_statement_list(file_path, else_body, source, violations);
                    }
                }
                Statement::Compound(compound) => {
                    self.check_statement_list(file_path, &compound.statements, source, violations);
                }
                _ => {}
            }
        }
    }

    /// Try to match: dest = g_bytes_get_data(bytes, ...); g_bytes_unref(bytes);
    fn try_bytes_pattern(
        &self,
        file_path: &std::path::Path,
        stmt1: &Statement,
        stmt2: &Statement,
        source: &[u8],
        violations: &mut Vec<Violation>,
    ) {
        // First statement: dest = g_bytes_get_data(bytes, &size)
        let Some((dest, bytes_var, size_arg, assignment, _call1)) =
            self.extract_bytes_get_data(stmt1, source)
        else {
            return;
        };

        // Second statement: g_bytes_unref(bytes)
        let Some(stmt2_end_byte) = self.extract_bytes_unref(stmt2, &bytes_var, source) else {
            return;
        };

        // Build the replacement
        let replacement = format!(
            "{} = g_bytes_unref_to_data ({}, {});",
            dest, bytes_var, size_arg
        );

        let fix = Fix::new(assignment.location.start_byte, stmt2_end_byte, replacement);

        violations.push(self.violation_with_fix(
            file_path,
            assignment.location.line,
            assignment.location.column,
            format!(
                "Use g_bytes_unref_to_data({}, {}) instead of g_bytes_get_data() followed by g_bytes_unref()",
                bytes_var, size_arg
            ),
            fix,
        ));
    }

    /// Extract components from: dest = g_bytes_get_data(bytes, size_arg)
    /// Returns (dest_text, bytes_var, size_arg, assignment, call)
    fn extract_bytes_get_data<'a>(
        &self,
        stmt: &'a Statement,
        source: &'a [u8],
    ) -> Option<(String, String, String, &'a Assignment, &'a CallExpression)> {
        let Statement::Expression(ExpressionStmt {
            expr: Expression::Assignment(assignment),
            ..
        }) = stmt
        else {
            return None;
        };

        // Check operator is "="
        if assignment.operator != AssignmentOp::Assign {
            return None;
        }

        // Right side should be a call to g_bytes_get_data
        let Expression::Call(call) = assignment.rhs.as_ref() else {
            return None;
        };

        if call.function != "g_bytes_get_data" {
            return None;
        }

        // Need exactly 2 arguments
        if call.arguments.len() != 2 {
            return None;
        }

        // Extract argument text from source
        let bytes_var = call.get_arg_text(0, source)?;
        let size_arg = call.get_arg_text(1, source)?;

        Some((
            assignment.lhs.clone(),
            bytes_var,
            size_arg,
            assignment,
            call,
        ))
    }

    /// Extract call from: g_bytes_unref(expected_var)
    /// Returns the end_byte of the statement (including semicolon)
    fn extract_bytes_unref(
        &self,
        stmt: &Statement,
        expected_var: &str,
        source: &[u8],
    ) -> Option<usize> {
        let Statement::Expression(expr_stmt) = stmt else {
            return None;
        };

        let Expression::Call(call) = &expr_stmt.expr else {
            return None;
        };

        if call.function != "g_bytes_unref" {
            return None;
        }

        // Need exactly 1 argument
        if call.arguments.len() != 1 {
            return None;
        }

        // Check argument matches expected variable
        let arg_text = call.get_arg_text(0, source)?;
        if arg_text != expected_var {
            return None;
        }

        Some(expr_stmt.location.end_byte)
    }
}
