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
        func: &gobject_ast::top_level::FunctionDefItem,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        let file = ast_context.project.files.get(path).unwrap();
        Statement::walk_pairs(&func.body_statements, &mut |stmt1, stmt2| {
            self.try_bytes_pattern(path, stmt1, stmt2, file, violations);
        });
    }
}

impl UseGBytesUnrefToData {
    /// Try to match: dest = g_bytes_get_data(bytes, ...); g_bytes_unref(bytes);
    fn try_bytes_pattern(
        &self,
        file_path: &std::path::Path,
        stmt1: &Statement,
        stmt2: &Statement,
        file: &gobject_ast::FileModel,
        violations: &mut Vec<Violation>,
    ) {
        // First statement: dest = g_bytes_get_data(bytes, &size)
        let Some((dest, bytes_var, size_arg, assignment, _call1)) =
            self.extract_bytes_get_data(stmt1, &file.source)
        else {
            return;
        };

        // Second statement: g_bytes_unref(bytes)
        if self
            .extract_bytes_unref(stmt2, &bytes_var, &file.source)
            .is_none()
        {
            return;
        };

        // Build the replacement
        let replacement = format!(
            "{} = g_bytes_unref_to_data ({}, {});",
            dest, bytes_var, size_arg
        );

        // Use two separate fixes to preserve comments between statements
        let fixes = vec![
            // Replace the first statement with the new call
            Fix::new(
                stmt1.location().start_byte,
                stmt1.location().end_byte,
                replacement.clone(),
            ),
            // Delete the entire second line
            Fix::delete_line(stmt2.location(), &file.source),
        ];

        violations.push(self.violation_with_fixes(
            file_path,
            assignment.location.line,
            assignment.location.column,
            format!(
                "Use g_bytes_unref_to_data({}, {}) instead of g_bytes_get_data() followed by g_bytes_unref()",
                bytes_var, size_arg
            ),
            fixes,
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

        if !call.is_function("g_bytes_get_data") {
            return None;
        }

        // Need exactly 2 arguments
        if call.arguments.len() != 2 {
            return None;
        }

        // Extract argument text from source
        let bytes_var = call.get_arg_text(0, source)?;
        let size_arg = call.get_arg_text(1, source)?;

        let dest_var = assignment.lhs_as_text();
        if dest_var.is_empty() {
            return None;
        }

        Some((dest_var, bytes_var, size_arg, assignment, call))
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

        if !call.is_function("g_bytes_unref") {
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
