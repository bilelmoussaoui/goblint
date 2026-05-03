use gobject_ast::{AssignmentOp, Expression, Statement};

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGClearSignalHandler;

impl Rule for UseGClearSignalHandler {
    fn name(&self) -> &'static str {
        "use_g_clear_signal_handler"
    }

    fn description(&self) -> &'static str {
        "Use g_clear_signal_handler() instead of g_signal_handler_disconnect() and zeroing the ID"
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
        // Walk through function body looking for patterns
        self.check_statements(&func.body_statements, file, path, violations);
    }
}

impl UseGClearSignalHandler {
    fn check_statements(
        &self,
        statements: &[Statement],
        file: &gobject_ast::FileModel,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        let mut i = 0;
        while i < statements.len() {
            // if (id) / if (id > 0) { disconnect; id = 0; } — replace entire if_statement
            if self.try_if_guarded(&statements[i], file_path, violations) {
                i += 1;
                continue;
            }

            // g_signal_handler_disconnect(obj, id); id = 0;
            if i + 1 < statements.len()
                && self.try_disconnect_then_zero(
                    &statements[i],
                    &statements[i + 1],
                    file,
                    file_path,
                    violations,
                )
            {
                i += 2;
                continue;
            }

            // bare g_signal_handler_disconnect(obj, struct->member) — no zero-assign
            if self.try_bare_disconnect_on_member(&statements[i], statements, file_path, violations)
            {
                i += 1;
                continue;
            }

            // Recursively check nested statements
            match &statements[i] {
                Statement::If(if_stmt) => {
                    self.check_statements(&if_stmt.then_body, file, file_path, violations);
                    if let Some(else_body) = &if_stmt.else_body {
                        self.check_statements(else_body, file, file_path, violations);
                    }
                }
                Statement::Compound(compound) => {
                    self.check_statements(&compound.statements, file, file_path, violations);
                }
                Statement::Labeled(labeled) => {
                    self.check_statements(
                        std::slice::from_ref(&labeled.statement),
                        file,
                        file_path,
                        violations,
                    );
                }
                _ => {}
            }

            i += 1;
        }
    }

    /// Matches `if (id) { g_signal_handler_disconnect(obj, id); id = 0; }`
    fn try_if_guarded(
        &self,
        stmt: &Statement,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) -> bool {
        let Statement::If(if_stmt) = stmt else {
            return false;
        };

        // Don't flag if there's an else branch
        if if_stmt.has_else() {
            return false;
        }

        // Extract the guarded ID from the condition
        let Some(guarded_id) = if_stmt.extract_nonzero_check_variable() else {
            return false;
        };

        // Body must have exactly 2 statements: disconnect call and zero assignment
        if if_stmt.then_body.len() != 2 {
            return false;
        }

        // First statement: g_signal_handler_disconnect(obj, id)
        let Some((obj, handler_id)) = self.extract_disconnect_args(&if_stmt.then_body[0]) else {
            return false;
        };

        // The guarded ID must match the disconnect's handler_id arg
        if handler_id != guarded_id {
            return false;
        }

        // Second statement: id = 0
        if !self.is_zero_assign(&if_stmt.then_body[1], &handler_id) {
            return false;
        }

        let replacement = format!("g_clear_signal_handler (&{handler_id}, {obj});");
        let fix = Fix::new(
            if_stmt.location.start_byte,
            if_stmt.location.end_byte,
            replacement.clone(),
        );

        violations.push(self.violation_with_fix(
            file_path,
            if_stmt.location.line,
            if_stmt.location.column,
            format!("Use {replacement} instead of if-guarded g_signal_handler_disconnect"),
            fix,
        ));
        true
    }

    /// Matches `g_signal_handler_disconnect(obj, id); id = 0;`
    fn try_disconnect_then_zero(
        &self,
        s1: &Statement,
        s2: &Statement,
        file: &gobject_ast::FileModel,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) -> bool {
        let Some((obj, handler_id)) = self.extract_disconnect_args(s1) else {
            return false;
        };

        if !self.is_zero_assign(s2, &handler_id) {
            return false;
        }

        let replacement = format!("g_clear_signal_handler (&{handler_id}, {obj});");

        // Use two separate fixes to preserve comments between statements
        let fixes = vec![
            // Replace the first statement with the new call
            Fix::new(
                s1.location().start_byte,
                s1.location().end_byte,
                replacement.clone(),
            ),
            // Delete the entire second line
            Fix::delete_line(s2.location(), &file.source),
        ];

        violations.push(self.violation_with_fixes(
            file_path,
            s1.location().line,
            s1.location().column,
            format!("Use {replacement} instead of g_signal_handler_disconnect and zeroing the ID"),
            fixes,
        ));
        true
    }

    /// Matches a bare `g_signal_handler_disconnect(obj, struct->member)` call
    fn try_bare_disconnect_on_member(
        &self,
        stmt: &Statement,
        all_stmts: &[Statement],
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) -> bool {
        let Some((obj, handler_id)) = self.extract_disconnect_args(stmt) else {
            return false;
        };

        // Only flag when the handler ID is a struct member access (contains ->)
        if !handler_id.contains("->") {
            return false;
        }

        // Extract the base pointer: `closure` from `closure->stopped_handler_id`.
        let base = handler_id.split("->").next().unwrap_or("").trim();
        if base.is_empty() {
            return false;
        }

        // Skip when the base struct or obj is freed in the same block
        if self.is_freed_in_stmts(all_stmts, base) || self.is_freed_in_stmts(all_stmts, &obj) {
            return false;
        }

        let replacement = format!("g_clear_signal_handler (&{handler_id}, {obj});");
        let fix = Fix::new(
            stmt.location().start_byte,
            stmt.location().end_byte,
            replacement.clone(),
        );

        violations.push(self.violation_with_fix(
            file_path,
            stmt.location().line,
            stmt.location().column,
            format!("Use {replacement} instead of g_signal_handler_disconnect (also zeroes the stored ID)"),
            fix,
        ));
        true
    }

    /// Extract `(obj, handler_id)` from a g_signal_handler_disconnect(obj, id)
    /// call
    fn extract_disconnect_args(&self, stmt: &Statement) -> Option<(String, String)> {
        let Statement::Expression(expr_stmt) = stmt else {
            return None;
        };

        let Expression::Call(call) = &expr_stmt.expr else {
            return None;
        };

        if !call.is_function("g_signal_handler_disconnect") {
            return None;
        }

        if call.arguments.len() != 2 {
            return None;
        }

        let obj = call.get_arg(0)?.extract_variable_name()?;
        let handler_id = call.get_arg(1)?.extract_variable_name()?;

        Some((obj, handler_id))
    }

    /// Check if stmt is `expected_id = 0;`
    fn is_zero_assign(&self, stmt: &Statement, expected_id: &str) -> bool {
        let Statement::Expression(expr_stmt) = stmt else {
            return false;
        };

        let Expression::Assignment(assign) = &expr_stmt.expr else {
            return false;
        };

        // Check left side matches expected_id and right side is 0
        assign.lhs_as_text() == expected_id
            && assign.operator == AssignmentOp::Assign
            && assign.rhs.is_zero()
    }

    /// Check if any statement calls a cleanup function on the target
    fn is_freed_in_stmts(&self, stmts: &[Statement], target: &str) -> bool {
        for stmt in stmts {
            let Statement::Expression(expr_stmt) = stmt else {
                continue;
            };

            let Expression::Call(call) = &expr_stmt.expr else {
                continue;
            };

            // Check if it's a cleanup function
            if !call.function_contains("free")
                && !call.function_contains("unref")
                && !call.function_contains("destroy")
                && !call.function_contains("clear")
            {
                continue;
            }

            // Check if any argument references the target
            for arg in &call.arguments {
                if self.arg_references(arg, target) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if an argument references the target variable
    fn arg_references(&self, arg: &gobject_ast::Argument, target: &str) -> bool {
        let gobject_ast::Argument::Expression(expr) = arg;

        let mut found = false;
        expr.walk(&mut |e| {
            match e {
                Expression::Identifier(id)
                    // Match both `target` and `&target`
                    if id.name == target => {
                        found = true;
                    }
                Expression::FieldAccess(f)
                    // Match field access like `self->source`
                    if f.text() == target => {
                        found = true;
                    }
                _ => {}
            }
        });
        found
    }
}
