use gobject_ast::{Expression, Statement};

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGClearHandleId;

impl Rule for UseGClearHandleId {
    fn name(&self) -> &'static str {
        "use_g_clear_handle_id"
    }

    fn description(&self) -> &'static str {
        "Suggest g_clear_handle_id instead of manual cleanup and zero assignment"
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
        self.check_statements(path, &func.body_statements, source, violations);
    }
}

impl UseGClearHandleId {
    fn check_statements(
        &self,
        file_path: &std::path::Path,
        statements: &[Statement],
        source: &[u8],
        violations: &mut Vec<Violation>,
    ) {
        // Check the statements themselves for cleanup pattern
        self.check_compound_statement(file_path, statements, source, violations);

        // Recurse into nested statements
        for stmt in statements {
            match stmt {
                Statement::If(if_stmt) => {
                    // Check if we can simplify/remove the if itself
                    // Returns true if it handled the then_body (found violations)
                    let handled = self.check_if_statement(file_path, if_stmt, source, violations);

                    // Only recurse into then_body if check_if_statement didn't handle it
                    if !handled {
                        self.check_statements(file_path, &if_stmt.then_body, source, violations);
                    }

                    if let Some(else_body) = &if_stmt.else_body {
                        self.check_statements(file_path, else_body, source, violations);
                    }
                }
                Statement::Compound(compound) => {
                    self.check_statements(file_path, &compound.statements, source, violations);
                }
                _ => {}
            }
        }
    }

    fn check_if_statement(
        &self,
        file_path: &std::path::Path,
        if_stmt: &gobject_ast::IfStatement,
        source: &[u8],
        violations: &mut Vec<Violation>,
    ) -> bool {
        let conversions = self.check_cleanup_then_zero(&if_stmt.then_body, source);

        if !conversions.is_empty() {
            let stmt_count = if_stmt.then_body.len();

            let has_else = if_stmt.else_body.is_some();
            let cond_id = self.extract_id_from_condition(&if_stmt.condition);

            for (var_name, cleanup_func, first_loc, second_loc) in conversions {
                let replacement = format!("g_clear_handle_id (&{}, {});", var_name, cleanup_func);

                let can_remove_if =
                    !has_else && cond_id.as_deref() == Some(var_name.as_str()) && stmt_count == 2;

                let fix = if can_remove_if {
                    Fix::new(
                        if_stmt.location.start_byte,
                        if_stmt.location.end_byte,
                        replacement.clone(),
                    )
                } else if stmt_count == 2 {
                    // Find braces around the statements
                    let first_start = if_stmt.then_body[0].location().start_byte;
                    let second_end = if_stmt.then_body[1].location().end_byte;
                    let (mut brace_start, brace_end) =
                        self.find_braces_around(first_start, second_end, source);

                    // Include the newline before the brace in the replacement
                    while brace_start > 0 && source[brace_start - 1] != b'\n' {
                        brace_start -= 1;
                    }
                    brace_start = brace_start.saturating_sub(1);

                    // Extract indentation from the brace line
                    let indent = self.get_indentation(brace_start + 1, source);
                    let formatted_replacement = format!("\n{}{}", indent, replacement);

                    Fix::new(brace_start, brace_end, formatted_replacement)
                } else {
                    Fix::new(
                        first_loc.start_byte,
                        second_loc.end_byte,
                        replacement.clone(),
                    )
                };

                violations.push(self.violation_with_fix(
                    file_path,
                    first_loc.line,
                    first_loc.column,
                    format!(
                        "Use {} instead of {} and zero assignment",
                        replacement, cleanup_func
                    ),
                    fix,
                ));
            }
            // We handled the cleanup pattern, return true to prevent double-checking
            return true;
        } else if if_stmt.then_body.len() == 1
            && if_stmt.then_has_braces
            && let Statement::Expression(expr_stmt) = &if_stmt.then_body[0]
            && let Expression::Call(call) = &expr_stmt.expr
            && call.function == "g_clear_handle_id"
        {
            let call_text = call.location.as_str(source).unwrap_or("");

            let loc = if_stmt.then_body[0].location();
            let fix = Fix::new(loc.start_byte, loc.end_byte, format!("{};", call_text));

            violations.push(self.violation_with_fix(
                file_path,
                if_stmt.location.line,
                if_stmt.location.column,
                "Remove unnecessary braces around single g_clear_handle_id call".to_string(),
                fix,
            ));
        }

        // Didn't find/handle cleanup pattern, let caller recurse into then_body
        false
    }

    fn check_compound_statement(
        &self,
        file_path: &std::path::Path,
        statements: &[Statement],
        source: &[u8],
        violations: &mut Vec<Violation>,
    ) {
        for (var_name, cleanup_func, first_loc, second_loc) in
            self.check_cleanup_then_zero(statements, source)
        {
            let replacement = format!("g_clear_handle_id (&{}, {});", var_name, cleanup_func);

            let fix = Fix::new(
                first_loc.start_byte,
                second_loc.end_byte,
                replacement.clone(),
            );

            violations.push(self.violation_with_fix(
                file_path,
                first_loc.line,
                first_loc.column,
                format!(
                    "Use {} instead of {} and zero assignment",
                    replacement, cleanup_func
                ),
                fix,
            ));
        }
    }

    fn check_cleanup_then_zero(
        &self,
        statements: &[Statement],
        source: &[u8],
    ) -> Vec<(
        String,
        String,
        gobject_ast::SourceLocation,
        gobject_ast::SourceLocation,
    )> {
        let mut results = Vec::new();

        for i in 0..statements.len().saturating_sub(1) {
            let first = &statements[i];
            let second = &statements[i + 1];

            if let Some((var_name, cleanup_func)) = self.extract_handle_cleanup(first, source)
                && let Some(assign_var) = self.extract_zero_assignment(second)
                && assign_var.trim() == var_name.trim()
            {
                results.push((
                    var_name,
                    cleanup_func,
                    first.location().clone(),
                    second.location().clone(),
                ));
            }
        }

        results
    }

    fn extract_handle_cleanup(&self, stmt: &Statement, source: &[u8]) -> Option<(String, String)> {
        let call = stmt.extract_call()?;

        let is_handle_cleanup = matches!(
            call.function.as_str(),
            "g_source_remove" | "g_source_destroy"
        );

        if !is_handle_cleanup || call.arguments.is_empty() {
            return None;
        }

        let gobject_ast::Argument::Expression(arg_expr) = &call.arguments[0];
        let var_name = arg_expr.location().as_str(source)?.trim().to_string();

        Some((var_name, call.function.clone()))
    }

    fn extract_zero_assignment(&self, stmt: &Statement) -> Option<String> {
        if let Statement::Expression(expr_stmt) = stmt
            && let Expression::Assignment(assign) = &expr_stmt.expr
            && assign.rhs.is_zero()
        {
            return Some(assign.lhs.clone());
        }
        None
    }

    fn extract_id_from_condition(&self, condition: &Expression) -> Option<String> {
        // Try direct variable extraction first
        if let Some(var) = condition.extract_variable_name() {
            return Some(var);
        }

        // Then try binary comparison
        if let Expression::Binary(bin) = condition {
            return bin.extract_compared_variable();
        }

        None
    }

    fn find_braces_around(&self, start: usize, _end: usize, source: &[u8]) -> (usize, usize) {
        // Search backwards from start to find '{'
        let mut brace_start = start;
        while brace_start > 0 && source[brace_start - 1] != b'{' {
            brace_start -= 1;
            if start - brace_start > 100 {
                break;
            }
        }
        if brace_start > 0 && source[brace_start - 1] == b'{' {
            brace_start -= 1;
        }

        // Search forwards from opening brace to find matching closing brace
        let mut brace_end = brace_start + 1;
        let mut depth = 1;
        while brace_end < source.len() && depth > 0 {
            if source[brace_end] == b'{' {
                depth += 1;
            } else if source[brace_end] == b'}' {
                depth -= 1;
            }
            brace_end += 1;
        }

        (brace_start, brace_end)
    }

    fn get_indentation(&self, pos: usize, source: &[u8]) -> String {
        // Find the start of the line
        let mut line_start = pos;
        while line_start > 0 && source[line_start - 1] != b'\n' {
            line_start -= 1;
        }

        // Extract whitespace from line start
        let mut indent = String::new();
        let mut i = line_start;
        while i < source.len() && (source[i] == b' ' || source[i] == b'\t') {
            indent.push(source[i] as char);
            i += 1;
        }

        indent
    }
}
