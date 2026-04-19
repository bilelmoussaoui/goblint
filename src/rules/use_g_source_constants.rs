use gobject_ast::{Expression, Statement};

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGSourceConstants;

impl Rule for UseGSourceConstants {
    fn name(&self) -> &'static str {
        "use_g_source_constants"
    }

    fn description(&self) -> &'static str {
        "Use G_SOURCE_CONTINUE/G_SOURCE_REMOVE instead of TRUE/FALSE in GSourceFunc callbacks"
    }

    fn category(&self) -> super::Category {
        super::Category::Style
    }

    fn fixable(&self) -> bool {
        true
    }

    fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        // Collect all callbacks passed to g_idle_add/g_timeout_add
        let mut callbacks_to_check: Vec<String> = Vec::new();

        for (_path, file) in ast_context.iter_c_files() {
            for func in file.iter_function_definitions() {
                // Find all source add calls (exclude _once variants as they return void)
                for call in func.find_calls(&[
                    "g_idle_add",
                    "g_idle_add_full",
                    "g_timeout_add",
                    "g_timeout_add_seconds",
                    "g_timeout_add_full",
                    "g_timeout_add_seconds_full",
                ]) {
                    if let Some(callback_name) = self.extract_callback_name(call, &file.source) {
                        callbacks_to_check.push(callback_name);
                    }
                }
            }
        }

        // Check each callback function for TRUE/FALSE returns
        for callback_name in callbacks_to_check {
            self.check_callback_returns(ast_context, &callback_name, violations);
        }
    }
}

impl UseGSourceConstants {
    fn extract_callback_name(
        &self,
        call: &gobject_ast::CallExpression,
        _source: &[u8],
    ) -> Option<String> {
        // Map of source-add function name → zero-based index of the GSourceFunc
        // argument
        let callback_arg_index: usize = match call.function.as_str() {
            "g_idle_add" => 0,
            "g_idle_add_full" | "g_timeout_add" | "g_timeout_add_seconds" => 1,
            "g_timeout_add_full" | "g_timeout_add_seconds_full" => 2,
            _ => return None,
        };

        if callback_arg_index >= call.arguments.len() {
            return None;
        }

        // Get the callback argument (should be an identifier)
        let arg_expr = call.get_arg(callback_arg_index)?;
        if let Expression::Identifier(id) = arg_expr {
            Some(id.name.clone())
        } else {
            None
        }
    }

    fn check_callback_returns(
        &self,
        ast_context: &AstContext,
        callback_name: &str,
        violations: &mut Vec<Violation>,
    ) {
        // Find the function definition
        for (path, file) in ast_context.iter_all_files() {
            for func in file.iter_function_definitions() {
                if func.name == callback_name {
                    self.check_statements(path, &func.body_statements, &file.source, violations);
                }
            }
        }
    }

    fn check_statements(
        &self,
        file_path: &std::path::Path,
        statements: &[Statement],
        source: &[u8],
        violations: &mut Vec<Violation>,
    ) {
        for stmt in statements {
            for ret_stmt in stmt.iter_returns() {
                if let Some(value) = &ret_stmt.value {
                    self.check_return_value(file_path, value, source, violations);
                }
            }
        }
    }

    fn check_return_value(
        &self,
        file_path: &std::path::Path,
        expr: &Expression,
        _source: &[u8],
        violations: &mut Vec<Violation>,
    ) {
        // Walk all nested expressions to find TRUE/FALSE
        expr.walk(&mut |e| match e {
            Expression::Identifier(id) if id.name == "TRUE" || id.name == "FALSE" => {
                let replacement = if id.name == "TRUE" {
                    "G_SOURCE_CONTINUE"
                } else {
                    "G_SOURCE_REMOVE"
                };

                let fix = Fix::new(
                    id.location.start_byte,
                    id.location.end_byte,
                    replacement.to_string(),
                );

                violations.push(self.violation_with_fix(
                    file_path,
                    id.location.line,
                    id.location.column,
                    format!(
                        "Use {} instead of {} in GSourceFunc callback",
                        replacement, id.name
                    ),
                    fix,
                ));
            }
            Expression::Boolean(b) => {
                let (old_name, replacement) = if b.value {
                    ("TRUE", "G_SOURCE_CONTINUE")
                } else {
                    ("FALSE", "G_SOURCE_REMOVE")
                };

                let fix = Fix::new(
                    b.location.start_byte,
                    b.location.end_byte,
                    replacement.to_string(),
                );

                violations.push(self.violation_with_fix(
                    file_path,
                    b.location.line,
                    b.location.column,
                    format!(
                        "Use {} instead of {} in GSourceFunc callback",
                        replacement, old_name
                    ),
                    fix,
                ));
            }
            _ => {}
        });
    }
}
