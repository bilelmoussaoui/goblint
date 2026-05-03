use gobject_ast::{Expression, Statement};

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGSourceOnce;

impl Rule for UseGSourceOnce {
    fn name(&self) -> &'static str {
        "use_g_source_once"
    }

    fn description(&self) -> &'static str {
        "Suggest using g_idle_add_once/g_timeout_add_once/g_timeout_add_seconds_once when callback always returns G_SOURCE_REMOVE"
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

        // Find g_idle_add, g_timeout_add, and g_timeout_add_seconds calls
        for call in func.find_calls(&["g_idle_add", "g_timeout_add", "g_timeout_add_seconds"]) {
            // Get the callback name from the first argument
            if let Some(callback_name) = self.extract_callback_name(call, source) {
                // Only proceed if callback is NOT used elsewhere
                if !self.is_callback_used_elsewhere(ast_context, &callback_name, path) {
                    // Find the callback function definition and check if all returns are
                    // FALSE/G_SOURCE_REMOVE
                    if let Some(callback_fixes) =
                        self.get_callback_fixes(ast_context, &callback_name, path)
                    {
                        let func_name = call.function_name();
                        let replacement = match func_name.as_str() {
                            "g_idle_add" => "g_idle_add_once",
                            "g_timeout_add_seconds" => "g_timeout_add_seconds_once",
                            _ => "g_timeout_add_once",
                        };

                        // Determine callback argument index
                        let callback_arg_index = if func_name == "g_idle_add" { 0 } else { 1 };

                        // Build arguments, replacing GSourceFunc cast with GSourceOnceFunc if
                        // present
                        let args_str = call
                            .arguments
                            .iter()
                            .enumerate()
                            .filter_map(|(idx, arg)| {
                                if idx == callback_arg_index {
                                    // Callback argument - replace cast type if present
                                    let gobject_ast::Argument::Expression(expr) = arg;
                                    if let Expression::Cast(cast) = &**expr
                                        && let Some(callback_name) =
                                            cast.operand.to_source_string(source)
                                    {
                                        return Some(format!(
                                            "(GSourceOnceFunc) {}",
                                            callback_name
                                        ));
                                    }
                                }
                                arg.to_source_string(source)
                            })
                            .collect::<Vec<_>>()
                            .join(", ");

                        // Fix 1: Replace g_idle_add → g_idle_add_once
                        let mut fixes = vec![Fix::new(
                            call.location.start_byte,
                            call.location.end_byte,
                            format!("{} ({})", replacement, args_str),
                        )];

                        // Add callback fixes (return type + return statements)
                        fixes.extend(callback_fixes);

                        violations.push(self.violation_with_fixes(
                            path,
                            call.location.line,
                            call.location.column,
                            format!(
                                "Callback '{}' always returns G_SOURCE_REMOVE. Use {} instead of {}",
                                callback_name, replacement, func_name
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
        // Determine which argument is the callback based on the function name
        // g_idle_add(callback, user_data) -> arg 0
        // g_timeout_add(interval, callback, user_data) -> arg 1
        // g_timeout_add_seconds(interval, callback, user_data) -> arg 1
        let func_name = call.function_name();
        let callback_arg_index = if func_name == "g_idle_add" {
            0
        } else {
            1 // g_timeout_add or g_timeout_add_seconds
        };

        let arg_expr = call.get_arg(callback_arg_index)?;

        // Handle direct identifier
        if let Expression::Identifier(id) = arg_expr {
            return Some(id.name.clone());
        }

        // Handle casted callback: (GSourceFunc) callback_name
        if let Expression::Cast(cast) = arg_expr
            && let Expression::Identifier(id) = &*cast.operand
        {
            return Some(id.name.clone());
        }

        None
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
        _file_path: &std::path::Path,
        func: &gobject_ast::top_level::FunctionDefItem,
        _ast_context: &AstContext,
    ) -> Option<Fix> {
        // Check if return type is gboolean
        if func.return_type.base_type != "gboolean" {
            return None;
        }

        // Use the location from the return type's TypeInfo
        Some(Fix::new(
            func.return_type.location.start_byte,
            func.return_type.location.end_byte,
            "void".to_string(),
        ))
    }

    fn fix_declaration_return_type(
        &self,
        _file_path: &std::path::Path,
        func: &gobject_ast::top_level::FunctionDeclItem,
        _ast_context: &AstContext,
    ) -> Option<Fix> {
        // Check if return type is gboolean
        if func.return_type.base_type != "gboolean" {
            return None;
        }

        // Preserve alignment by padding "void" to match the original type length
        let replacement = format!(
            "{:width$}",
            "void",
            width = func.return_type.full_text.trim().len()
        );

        // Use the location from the return type's TypeInfo
        Some(Fix::new(
            func.return_type.location.start_byte,
            func.return_type.location.end_byte,
            replacement,
        ))
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
                if !self.is_source_add_statement(s, callback_name)
                    && s.expressions()
                        .iter()
                        .any(|e| e.contains_identifier(callback_name))
                {
                    found = true;
                }
            });
            if found {
                return true;
            }
        }
        false
    }

    fn is_source_add_statement(&self, stmt: &Statement, callback_name: &str) -> bool {
        // Check if this statement is a g_idle_add/g_timeout_add/g_timeout_add_seconds
        // call with our callback
        if let Statement::Expression(expr_stmt) = stmt
            && expr_stmt.expr.is_call_to_any(&[
                "g_idle_add",
                "g_timeout_add",
                "g_timeout_add_seconds",
            ])
            && let Expression::Call(call) = &expr_stmt.expr
        {
            // Determine which argument is the callback
            let func_name = call.function_name();
            let callback_arg_index = if func_name == "g_idle_add" { 0 } else { 1 };

            if let Some(arg_expr) = call.get_arg(callback_arg_index) {
                // Handle direct identifier
                if let Expression::Identifier(id) = arg_expr {
                    return id.name == callback_name;
                }
                // Handle casted callback: (GSourceFunc) callback_name
                if let Expression::Cast(cast) = arg_expr
                    && let Expression::Identifier(id) = &*cast.operand
                {
                    return id.name == callback_name;
                }
            }
        }
        false
    }
}
