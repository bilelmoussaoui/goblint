use std::path::Path;

use gobject_ast::{Expression, Statement, UnaryOp};

use crate::{
    ast_context::AstContext,
    config::Config,
    rules::{Category, Rule, Violation},
};

pub struct GErrorLeak;

impl Rule for GErrorLeak {
    fn name(&self) -> &'static str {
        "g_error_leak"
    }

    fn description(&self) -> &'static str {
        "Check for GError variables that are neither freed nor propagated"
    }

    fn category(&self) -> Category {
        Category::Correctness
    }

    fn check_func_impl(
        &self,
        _ast_context: &AstContext,
        _config: &Config,
        func: &gobject_ast::top_level::FunctionDefItem,
        path: &Path,
        violations: &mut Vec<Violation>,
    ) {
        // Find all local GError* variables initialized to NULL
        let mut gerror_vars = Vec::new();

        for stmt in &func.body_statements {
            for decl in stmt.iter_declarations() {
                // Check if it's a GError* variable initialized to NULL
                if is_gerror_pointer(&decl.type_name)
                    && decl.initializer.as_ref().is_some_and(|init| init.is_null())
                {
                    gerror_vars.push((decl.name.clone(), decl.location));
                }
            }
        }

        // For each GError* variable, check if it's properly handled
        for (var_name, loc) in gerror_vars {
            // Check if the variable is actually used (passed to functions as &error)
            let is_used = is_error_used(&func.body_statements, &var_name);

            if !is_used {
                continue; // Not used, so no leak
            }

            // Check if it's properly handled (freed or propagated)
            let is_freed = is_error_freed(&func.body_statements, &var_name);
            let is_propagated = is_error_propagated(&func.body_statements, &var_name);

            // If the function contains noreturn calls (g_error, g_assert, etc.),
            // skip the leak check as the program will terminate anyway
            let has_noreturn = calls_noreturn_function(&func.body_statements);

            if !is_freed && !is_propagated && !has_noreturn {
                violations.push(self.violation(
                    path,
                    loc.line,
                    loc.column,
                    format!(
                        "GError variable '{}' may be leaked; it should be freed with g_error_free/g_clear_error or propagated with g_propagate_error/g_task_return_error/g_steal_pointer",
                        var_name
                    ),
                ));
            }
        }
    }
}

/// Check if a type is a GError pointer
fn is_gerror_pointer(type_name: &str) -> bool {
    let normalized = type_name.replace(' ', "");
    normalized.contains("GError*") || normalized == "GError**"
}

/// Check if the function calls a non-returning function (g_error, g_assert,
/// exit, etc.) that would terminate the program, making error cleanup
/// unnecessary
fn calls_noreturn_function(statements: &[Statement]) -> bool {
    let noreturn_functions = [
        "g_error",
        "g_assert",
        "g_assert_not_reached",
        "g_return_if_fail",
        "g_return_val_if_fail",
        "exit",
        "abort",
        "_exit",
    ];

    for stmt in statements {
        for call in stmt.iter_calls() {
            if noreturn_functions.contains(&call.function.as_str()) {
                return true;
            }
        }
    }
    false
}

/// Check if the error variable is used (passed to functions as &error)
fn is_error_used(statements: &[Statement], var_name: &str) -> bool {
    for stmt in statements {
        let mut found = false;
        stmt.walk_expressions(&mut |expr| {
            // Recursively walk ALL nested expressions
            expr.walk(&mut |nested_expr| {
                // Check for &error pattern (address-of operator)
                if let Expression::Unary(unary) = nested_expr
                    && unary.operator == UnaryOp::AddressOf
                    && let Expression::Identifier(id) = &*unary.operand
                    && id.name == var_name
                {
                    found = true;
                }
            });
        });
        if found {
            return true;
        }
    }
    false
}

/// Check if the error variable is freed (g_error_free or g_clear_error)
fn is_error_freed(statements: &[Statement], var_name: &str) -> bool {
    check_error_handled(statements, var_name, &["g_error_free", "g_clear_error"])
}

/// Check if the error variable is propagated (g_propagate_error,
/// g_steal_pointer, g_task_return_error, etc.)
fn is_error_propagated(statements: &[Statement], var_name: &str) -> bool {
    // Check for known ownership-transfer functions
    if check_error_handled(
        statements,
        var_name,
        &[
            "g_propagate_error",
            "g_steal_pointer",
            "g_task_return_error",
        ],
    ) {
        return true;
    }

    // Check for common naming patterns that indicate ownership transfer or
    // termination Functions like *_terminate_with_error, *_set_error, etc.
    for stmt in statements {
        for call in stmt.iter_calls() {
            let func_name = call.function.as_str();
            // Common patterns for functions that take ownership or terminate
            if func_name.contains("_terminate_") && func_name.contains("error")
                || func_name.ends_with("_set_error")
                || func_name.contains("_set_g_error")
            {
                // Check if the error variable is in the arguments
                for arg in &call.arguments {
                    let gobject_ast::expression::Argument::Expression(arg_expr) = arg;
                    if arg_expr.contains_identifier(var_name) {
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// Check if the error variable is handled by specific functions
fn check_error_handled(statements: &[Statement], var_name: &str, functions: &[&str]) -> bool {
    for stmt in statements {
        for call in stmt.iter_calls() {
            if functions.contains(&call.function.as_str()) {
                // Check if the error variable is in the arguments
                for arg in &call.arguments {
                    let gobject_ast::expression::Argument::Expression(arg_expr) = arg;
                    // Could be passed as `error` or `&error`
                    if arg_expr.contains_identifier(var_name) {
                        return true;
                    }
                }
            }
        }
    }
    false
}
