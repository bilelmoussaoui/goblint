use gobject_ast::{Expression, Statement};

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGSourceOnce;

impl Rule for UseGSourceOnce {
    fn name(&self) -> &'static str {
        "use_g_source_once"
    }

    fn description(&self) -> &'static str {
        "Suggest using g_idle_add_once/g_timeout_add_once when callback always returns G_SOURCE_REMOVE"
    }

    fn category(&self) -> super::Category {
        super::Category::Style
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

        // Find g_idle_add and g_timeout_add calls
        for call in func.find_calls(&["g_idle_add", "g_timeout_add"]) {
            // Get the callback name from the first argument
            if let Some(callback_name) = self.extract_callback_name(call, source) {
                // Only proceed if callback is NOT used elsewhere
                if !self.is_callback_used_elsewhere(ast_context, &callback_name, path) {
                    // Find the callback function definition and check if all returns are
                    // FALSE/G_SOURCE_REMOVE
                    if let Some(callback_fixes) =
                        self.get_callback_fixes(ast_context, &callback_name, path)
                    {
                        let replacement = if call.function == "g_idle_add" {
                            "g_idle_add_once"
                        } else {
                            "g_timeout_add_once"
                        };

                        // Fix 1: Replace g_idle_add → g_idle_add_once
                        let mut fixes = vec![Fix::new(
                            call.location.start_byte,
                            call.location.end_byte,
                            format!(
                                "{} ({})",
                                replacement,
                                call.arguments
                                    .iter()
                                    .filter_map(|arg| arg.to_source_string(source))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                        )];

                        // Add callback fixes (return type + return statements)
                        fixes.extend(callback_fixes);

                        violations.push(self.violation_with_fixes(
                            path,
                            call.location.line,
                            call.location.column,
                            format!(
                                "Callback '{}' always returns G_SOURCE_REMOVE. Use {} instead of {}",
                                callback_name, replacement, call.function
                            ),
                            fixes,
                        ));
                    }
                }
            }
        }
    }
}

impl UseGSourceOnce {
    fn extract_callback_name(
        &self,
        call: &gobject_ast::CallExpression,
        _source: &[u8],
    ) -> Option<String> {
        // Get the first argument (the callback function)
        if call.arguments.is_empty() {
            return None;
        }

        let gobject_ast::Argument::Expression(arg_expr) = &call.arguments[0];
        if let Expression::Identifier(id) = arg_expr.as_ref() {
            Some(id.name.clone())
        } else {
            None
        }
    }

    fn get_callback_fixes(
        &self,
        ast_context: &AstContext,
        callback_name: &str,
        target_file: &std::path::Path,
    ) -> Option<Vec<Fix>> {
        let mut fixes = Vec::new();
        let mut found_definition = false;

        // Find the function definition and all declarations in the same file
        for (path, file) in ast_context.iter_all_files() {
            // Only process callbacks in the same file
            if path != target_file {
                continue;
            }

            for func in &file.functions {
                if func.name != callback_name {
                    continue;
                }

                if func.is_definition {
                    // Check if all returns are FALSE/G_SOURCE_REMOVE/0
                    let return_values = self.collect_all_return_values(&func.body_statements);

                    // Must have at least one return statement
                    if return_values.is_empty() {
                        return None;
                    }

                    // All returns must be FALSE or G_SOURCE_REMOVE or 0
                    if !return_values
                        .iter()
                        .all(|r| r == "FALSE" || r == "G_SOURCE_REMOVE" || r == "0" || r == "false")
                    {
                        return None;
                    }

                    // Fix: Change return type from gboolean to void in definition
                    // We need to find "gboolean" in the function and replace it
                    if let Some(fix) = self.fix_definition_return_type(path, func, ast_context) {
                        fixes.push(fix);
                    }

                    // Fix: Remove all return statements (entire lines)
                    let return_locations = self.collect_all_return_locations(&func.body_statements);
                    for location in return_locations {
                        let (line_start, line_end) =
                            self.find_line_bounds(location.start_byte, &file.source);
                        fixes.push(Fix::new(line_start, line_end, String::new()));
                    }

                    found_definition = true;
                } else {
                    // This is a declaration - fix by searching the line in the file
                    if let Some(fix) = self.fix_declaration_return_type(path, func, ast_context) {
                        fixes.push(fix);
                    }
                }
            }
        }

        if found_definition && !fixes.is_empty() {
            Some(fixes)
        } else {
            None
        }
    }

    fn collect_all_return_values(&self, statements: &[Statement]) -> Vec<String> {
        let mut returns = Vec::new();

        for stmt in statements {
            stmt.walk(&mut |s| {
                if let Statement::Return(ret_stmt) = s
                    && let Some(value) = &ret_stmt.value
                {
                    // Get the return value as a string
                    if let Some(val_str) = self.expr_to_simple_string(value) {
                        returns.push(val_str);
                    }
                }
            });
        }

        returns
    }

    fn collect_all_return_locations(
        &self,
        statements: &[Statement],
    ) -> Vec<gobject_ast::SourceLocation> {
        let mut locations = Vec::new();

        for stmt in statements {
            stmt.walk(&mut |s| {
                if let Statement::Return(ret_stmt) = s {
                    locations.push(ret_stmt.location.clone());
                }
            });
        }

        locations
    }

    fn expr_to_simple_string(&self, expr: &Expression) -> Option<String> {
        match expr {
            Expression::Identifier(id) => Some(id.name.clone()),
            Expression::NumberLiteral(n) => Some(n.value.clone()),
            Expression::Boolean(b) => Some(if b.value {
                "true".to_string()
            } else {
                "false".to_string()
            }),
            _ => None,
        }
    }

    fn fix_definition_return_type(
        &self,
        file_path: &std::path::Path,
        func: &gobject_ast::FunctionInfo,
        ast_context: &AstContext,
    ) -> Option<Fix> {
        // Get the file source
        let file = ast_context.project.files.get(file_path)?;
        let source = &file.source;

        // Find "gboolean" in the function signature
        let func_start = func.start_byte?;
        let func_end = func.end_byte?;

        // Search for "gboolean" before the function body
        let func_text = std::str::from_utf8(&source[func_start..func_end]).ok()?;

        // Find the position of the function body (first '{')
        let body_start = func_text.find('{')?;
        let signature = &func_text[..body_start];

        if let Some(offset) = signature.find("gboolean") {
            let gboolean_start = func_start + offset;
            let gboolean_end = gboolean_start + "gboolean".len();

            return Some(Fix::new(gboolean_start, gboolean_end, "void".to_string()));
        }

        None
    }

    fn fix_declaration_return_type(
        &self,
        file_path: &std::path::Path,
        func: &gobject_ast::FunctionInfo,
        ast_context: &AstContext,
    ) -> Option<Fix> {
        // Get the file source
        let file = ast_context.project.files.get(file_path)?;
        let source = &file.source;

        // Find the line where the declaration is
        let mut line_start = 0;
        let mut current_line = 1;

        for (i, &byte) in source.iter().enumerate() {
            if current_line == func.line {
                // Found the line, now find "gboolean" on this line
                let mut line_end = i;
                while line_end < source.len() && source[line_end] != b'\n' {
                    line_end += 1;
                }

                let line_bytes = &source[line_start..line_end];
                let line_str = std::str::from_utf8(line_bytes).unwrap_or("");

                // Search for "gboolean" in the line
                if let Some(offset) = line_str.find("gboolean") {
                    let gboolean_start = line_start + offset;
                    let gboolean_end = gboolean_start + "gboolean".len();

                    // Preserve alignment by padding "void" to match "gboolean" length
                    let replacement = format!("{:8}", "void"); // "gboolean" is 8 chars

                    return Some(Fix::new(gboolean_start, gboolean_end, replacement));
                }

                return None;
            }

            if byte == b'\n' {
                current_line += 1;
                line_start = i + 1;
            }
        }

        None
    }

    fn is_callback_used_elsewhere(
        &self,
        ast_context: &AstContext,
        callback_name: &str,
        file_path: &std::path::Path,
    ) -> bool {
        // Search the file for all uses of this callback name
        for (path, file) in ast_context.iter_c_files() {
            if path != file_path {
                continue;
            }

            for func in &file.functions {
                if !func.is_definition {
                    continue;
                }

                // Walk through all statements looking for uses of the callback name
                if self.has_non_source_add_usage(&func.body_statements, callback_name) {
                    return true;
                }
            }
        }

        false
    }

    fn has_non_source_add_usage(&self, statements: &[Statement], callback_name: &str) -> bool {
        for stmt in statements {
            let mut found = false;
            stmt.walk(&mut |s| {
                if self.statement_uses_identifier(s, callback_name) {
                    // Check if this is inside a g_idle_add or g_timeout_add call
                    // For now, we'll be conservative and just check if it's used in any expression
                    // that's not a direct g_idle_add/g_timeout_add call
                    if !self.is_source_add_statement(s, callback_name) {
                        found = true;
                    }
                }
            });
            if found {
                return true;
            }
        }
        false
    }

    fn statement_uses_identifier(&self, stmt: &Statement, identifier: &str) -> bool {
        match stmt {
            Statement::Expression(expr_stmt) => {
                self.expr_uses_identifier(&expr_stmt.expr, identifier)
            }
            Statement::Return(ret_stmt) => {
                if let Some(value) = &ret_stmt.value {
                    self.expr_uses_identifier(value, identifier)
                } else {
                    false
                }
            }
            Statement::Declaration(decl) => {
                if let Some(init) = &decl.initializer {
                    self.expr_uses_identifier(init, identifier)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn expr_uses_identifier(&self, expr: &Expression, identifier: &str) -> bool {
        let mut found = false;
        expr.walk(&mut |e| {
            if let Expression::Identifier(id) = e
                && id.name == identifier
            {
                found = true;
            }
        });
        found
    }

    fn is_source_add_statement(&self, stmt: &Statement, callback_name: &str) -> bool {
        // Check if this statement is a g_idle_add/g_timeout_add call with our callback
        if let Statement::Expression(expr_stmt) = stmt
            && let Expression::Call(call) = &expr_stmt.expr
            && (call.function == "g_idle_add" || call.function == "g_timeout_add")
            && !call.arguments.is_empty()
        {
            let gobject_ast::Argument::Expression(arg_expr) = &call.arguments[0];
            if let Expression::Identifier(id) = arg_expr.as_ref() {
                return id.name == callback_name;
            }
        }
        false
    }

    fn find_line_bounds(&self, start_byte: usize, source: &[u8]) -> (usize, usize) {
        // Find the start of the line
        let mut line_start = start_byte;
        while line_start > 0 && source[line_start - 1] != b'\n' {
            line_start -= 1;
        }

        // Check if the previous line is empty (only whitespace)
        if line_start > 0 {
            let mut prev_line_start = line_start - 1; // Skip the '\n'
            while prev_line_start > 0 && source[prev_line_start - 1] != b'\n' {
                prev_line_start -= 1;
            }

            // Check if the line is only whitespace
            let prev_line = &source[prev_line_start..line_start - 1];
            if prev_line.iter().all(|&b| b == b' ' || b == b'\t') {
                line_start = prev_line_start;
            }
        }

        // Find the end of the line (including newline)
        let mut line_end = start_byte;
        while line_end < source.len() && source[line_end] != b'\n' {
            line_end += 1;
        }
        // Include the newline character
        if line_end < source.len() && source[line_end] == b'\n' {
            line_end += 1;
        }

        (line_start, line_end)
    }
}
