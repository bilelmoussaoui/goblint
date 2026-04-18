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

        let arg_expr = call.get_arg(0)?;
        if let Expression::Identifier(id) = arg_expr {
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

            // Check function definitions
            for func in file.iter_function_definitions() {
                if func.name != callback_name {
                    continue;
                }

                // Check if all returns are FALSE/G_SOURCE_REMOVE/0
                let return_exprs = func.collect_return_values();

                // Must have at least one return statement
                if return_exprs.is_empty() {
                    return None;
                }

                // All returns must be FALSE or G_SOURCE_REMOVE or 0
                if !return_exprs.iter().all(|expr| {
                    expr.to_simple_string().is_some_and(|s| {
                        s == "FALSE" || s == "G_SOURCE_REMOVE" || s == "0" || s == "false"
                    })
                }) {
                    return None;
                }

                // Fix: Change return type from gboolean to void in definition
                // We need to find "gboolean" in the function and replace it
                if let Some(fix) = self.fix_definition_return_type(path, func, ast_context) {
                    fixes.push(fix);
                }

                // Fix: Remove all return statements (entire lines)
                for ret_expr in return_exprs {
                    let (line_start, line_end) = ret_expr.location().find_line_bounds(&file.source);
                    fixes.push(Fix::new(line_start, line_end, String::new()));
                }

                found_definition = true;
            }

            // Check function declarations
            for func in file.iter_function_declarations() {
                if func.name != callback_name {
                    continue;
                }

                // This is a declaration - fix by searching the line in the file
                if let Some(fix) = self.fix_declaration_return_type(path, func, ast_context) {
                    fixes.push(fix);
                }
            }
        }

        if found_definition && !fixes.is_empty() {
            Some(fixes)
        } else {
            None
        }
    }

    fn fix_definition_return_type(
        &self,
        file_path: &std::path::Path,
        func: &gobject_ast::top_level::FunctionDefItem,
        ast_context: &AstContext,
    ) -> Option<Fix> {
        // Get the file source
        let file = ast_context.project.files.get(file_path)?;
        let source = &file.source;

        // Find "gboolean" in the function signature using location
        let func_start = func.location.start_byte;
        let func_end = func.location.end_byte;

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
        func: &gobject_ast::top_level::FunctionDeclItem,
        ast_context: &AstContext,
    ) -> Option<Fix> {
        // Get the file source
        let file = ast_context.project.files.get(file_path)?;
        let source = &file.source;

        // Find the line where the declaration is
        let mut line_start = 0;
        let mut current_line = 1;

        for (i, &byte) in source.iter().enumerate() {
            if current_line == func.location.line {
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

            for func in file.iter_function_definitions() {
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
            && let Some(arg_expr) = call.get_arg(0)
            && let Expression::Identifier(id) = arg_expr
        {
            return id.name == callback_name;
        }
        false
    }
}
