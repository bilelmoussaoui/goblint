use std::collections::HashSet;

use gobject_ast::{Expression, Statement, UnaryOp};

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGFileLoadBytes;

impl Rule for UseGFileLoadBytes {
    fn name(&self) -> &'static str {
        "use_g_file_load_bytes"
    }

    fn description(&self) -> &'static str {
        "Suggest g_file_load_bytes/g_file_load_bytes_async instead of g_file_load_contents + g_bytes_new_take"
    }

    fn category(&self) -> super::Category {
        super::Category::Complexity
    }

    fn fixable(&self) -> bool {
        false // Complex pattern, needs manual review
    }

    fn check_func_impl(
        &self,
        _ast_context: &AstContext,
        _config: &Config,
        func: &gobject_ast::FunctionInfo,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        if !func.is_definition {
            return;
        }

        self.check_function(func, path, violations);
    }
}

impl UseGFileLoadBytes {
    fn check_function(
        &self,
        func: &gobject_ast::FunctionInfo,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        // Find all g_file_load_contents calls and track their output variables
        let load_contents_vars = self.find_load_contents_vars(func);

        // Find all g_bytes_new_take calls that use those variables
        self.find_bytes_new_take_violations(
            &func.body_statements,
            file_path,
            &load_contents_vars,
            violations,
        );
    }

    /// Find all g_file_load_contents calls and return the set of variables they
    /// populate
    fn find_load_contents_vars(&self, func: &gobject_ast::FunctionInfo) -> HashSet<String> {
        let mut result = HashSet::new();

        // Find all g_file_load_contents or g_file_load_contents_finish calls
        for call in func.find_calls(&["g_file_load_contents", "g_file_load_contents_finish"]) {
            // g_file_load_contents(file, cancellable, &contents, &length, &etag, &error)
            //                      0     1            2          3         4       5
            if call.arguments.len() >= 6 {
                // Extract the contents variable from argument 2 (&contents)
                if let Some(contents_var) = self.extract_pointer_var(&call.arguments[2]) {
                    result.insert(contents_var);
                }
            }
        }

        result
    }

    fn find_bytes_new_take_violations(
        &self,
        statements: &[Statement],
        file_path: &std::path::Path,
        load_contents_vars: &HashSet<String>,
        violations: &mut Vec<Violation>,
    ) {
        for stmt in statements {
            stmt.walk(&mut |s| match s {
                Statement::Expression(expr_stmt) => {
                    self.check_expr_for_bytes_new_take(
                        &expr_stmt.expr,
                        file_path,
                        load_contents_vars,
                        violations,
                    );
                }
                Statement::Declaration(decl) => {
                    if let Some(init) = &decl.initializer {
                        self.check_expr_for_bytes_new_take(
                            init,
                            file_path,
                            load_contents_vars,
                            violations,
                        );
                    }
                }
                Statement::Return(ret) => {
                    if let Some(expr) = &ret.value {
                        self.check_expr_for_bytes_new_take(
                            expr,
                            file_path,
                            load_contents_vars,
                            violations,
                        );
                    }
                }
                _ => {}
            });
        }
    }

    fn check_expr_for_bytes_new_take(
        &self,
        expr: &Expression,
        file_path: &std::path::Path,
        load_contents_vars: &HashSet<String>,
        violations: &mut Vec<Violation>,
    ) {
        if let Expression::Call(call) = expr
            && call.function == "g_bytes_new_take"
            && call.arguments.len() >= 2
        {
            // Extract the first argument (contents variable)
            if let Some(contents_var) = self.extract_contents_var(&call.arguments[0]) {
                // Check if this contents variable came from g_file_load_contents
                if load_contents_vars.contains(&contents_var) {
                    violations.push(self.violation(
                            file_path,
                            call.location.line,
                            call.location.column,
                            "Consider using g_file_load_bytes/g_file_load_bytes_async instead of g_file_load_contents + g_bytes_new_take for simplicity".to_string(),
                        ));
                }
            }
        }
    }

    /// Extract variable name from &var argument
    fn extract_pointer_var(&self, arg: &gobject_ast::Argument) -> Option<String> {
        let gobject_ast::Argument::Expression(expr) = arg;

        // Handle &var
        if let Expression::Unary(unary) = expr.as_ref()
            && unary.operator == UnaryOp::AddressOf
        {
            return unary.operand.extract_variable_name();
        }

        None
    }

    /// Extract variable name from first argument of g_bytes_new_take
    /// Handles: contents, g_steal_pointer(&contents)
    fn extract_contents_var(&self, arg: &gobject_ast::Argument) -> Option<String> {
        let gobject_ast::Argument::Expression(expr) = arg;

        match expr.as_ref() {
            // Direct variable: contents
            Expression::Identifier(id) => Some(id.name.clone()),
            Expression::FieldAccess(f) => Some(f.text.clone()),
            // g_steal_pointer(&contents)
            Expression::Call(call) => {
                if call.function == "g_steal_pointer" && !call.arguments.is_empty() {
                    self.extract_pointer_var(&call.arguments[0])
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
