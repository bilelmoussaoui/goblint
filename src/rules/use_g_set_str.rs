use gobject_ast::{AssignmentOp, Expression, Statement, UnaryOp};

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGSetStr;

impl Rule for UseGSetStr {
    fn name(&self) -> &'static str {
        "use_g_set_str"
    }

    fn description(&self) -> &'static str {
        "Suggest g_set_str() instead of manual g_free and g_strdup"
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

        // Get the source for this function to preserve comments
        if let Some(func_source) = ast_context.get_function_source(path, func) {
            self.check_statements(
                &func.body_statements,
                path,
                func_source,
                func.start_byte.unwrap_or(0),
                violations,
            );
        }
    }
}

impl UseGSetStr {
    fn check_statements(
        &self,
        statements: &[Statement],
        file_path: &std::path::Path,
        source: &[u8],
        base_byte: usize,
        violations: &mut Vec<Violation>,
    ) {
        let mut i = 0;
        while i < statements.len() {
            // Check for g_free(var) or g_clear_pointer(&var, g_free) pattern
            if i + 1 < statements.len()
                && self.try_free_then_strdup(
                    &statements[i],
                    &statements[i + 1],
                    file_path,
                    source,
                    base_byte,
                    violations,
                )
            {
                i += 2;
                continue;
            }

            // Recurse into nested statements
            match &statements[i] {
                Statement::If(if_stmt) => {
                    self.check_statements(
                        &if_stmt.then_body,
                        file_path,
                        source,
                        base_byte,
                        violations,
                    );
                    if let Some(else_body) = &if_stmt.else_body {
                        self.check_statements(else_body, file_path, source, base_byte, violations);
                    }
                }
                Statement::Compound(compound) => {
                    self.check_statements(
                        &compound.statements,
                        file_path,
                        source,
                        base_byte,
                        violations,
                    );
                }
                Statement::Labeled(labeled) => {
                    self.check_statements(
                        std::slice::from_ref(&labeled.statement),
                        file_path,
                        source,
                        base_byte,
                        violations,
                    );
                }
                _ => {}
            }

            i += 1;
        }
    }

    /// Check for g_free(var) followed by var = g_strdup(...)
    fn try_free_then_strdup(
        &self,
        s1: &Statement,
        s2: &Statement,
        file_path: &std::path::Path,
        source: &[u8],
        base_byte: usize,
        violations: &mut Vec<Violation>,
    ) -> bool {
        // First statement: g_free(var) or g_clear_pointer(&var, g_free)
        let Some(var_name) = self.extract_gfree_var(s1) else {
            return false;
        };

        // Second statement: var = g_strdup(...)
        let Some((assign_var, new_val)) = self.extract_strdup_assignment(s2) else {
            return false;
        };

        if assign_var != var_name {
            return false;
        }

        let set_str_call = format!("g_set_str (&{var_name}, {new_val});");

        // Extract bytes between the two statements to preserve comments
        let s1_end = s1.location().end_byte - base_byte;
        let s2_start = s2.location().start_byte - base_byte;
        let intermediate = std::str::from_utf8(&source[s1_end..s2_start]).unwrap_or("");
        let comment_prefix = intermediate.trim_start_matches(['\n', '\r', ' ', '\t']);

        // If there are comments, include them in the fix
        let fix_text = if comment_prefix.is_empty() {
            set_str_call.clone()
        } else {
            format!("{}{}", comment_prefix, set_str_call)
        };

        let fix = Fix::new(s1.location().start_byte, s2.location().end_byte, fix_text);

        violations.push(self.violation_with_fix(
            file_path,
            s1.location().line,
            s1.location().column,
            format!("Use {set_str_call} instead of g_free and g_strdup"),
            fix,
        ));
        true
    }

    /// Extract variable from g_free(var) or g_clear_pointer(&var, g_free)
    fn extract_gfree_var(&self, stmt: &Statement) -> Option<String> {
        let Statement::Expression(expr_stmt) = stmt else {
            return None;
        };

        let Expression::Call(call) = &expr_stmt.expr else {
            return None;
        };

        if call.function == "g_free" {
            // g_free(var) - var can be identifier, field access, or *ptr
            if call.arguments.is_empty() {
                return None;
            }
            let var = self.arg_to_string(&call.arguments[0]);
            return if var.is_empty() { None } else { Some(var) };
        } else if call.function == "g_clear_pointer" {
            // g_clear_pointer(&var, g_free)
            if call.arguments.len() != 2 {
                return None;
            }

            // Check if second argument is g_free
            let gobject_ast::Argument::Expression(second_arg) = &call.arguments[1];
            if let Expression::Identifier(id) = &**second_arg {
                if id.name != "g_free" {
                    return None;
                }
            } else {
                return None;
            }

            // First argument is &var - extract var
            let gobject_ast::Argument::Expression(first_arg) = &call.arguments[0];
            if let Expression::Unary(unary) = &**first_arg
                && unary.operator == UnaryOp::AddressOf
            {
                return Some(self.expr_to_string(&unary.operand));
            }
        }

        None
    }

    /// Extract (var, new_val) from var = g_strdup(new_val) or var = cond ?
    /// g_strdup(...) : NULL
    fn extract_strdup_assignment(&self, stmt: &Statement) -> Option<(String, String)> {
        let Statement::Expression(expr_stmt) = stmt else {
            return None;
        };

        let Expression::Assignment(assign) = &expr_stmt.expr else {
            return None;
        };

        if assign.operator != AssignmentOp::Assign {
            return None;
        }

        // Direct g_strdup call: var = g_strdup(new_val)
        if let Expression::Call(call) = &*assign.rhs
            && call.function == "g_strdup"
            && !call.arguments.is_empty()
        {
            let new_val = self.arg_to_string(&call.arguments[0]);
            return Some((assign.lhs.clone(), new_val));
        }

        // Ternary: var = cond ? g_strdup(...) : NULL
        if let Expression::Conditional(cond) = &*assign.rhs
            && let Expression::Call(call) = &*cond.then_expr
            && (call.function == "g_strdup" || call.function == "g_strndup")
        {
            // Use the condition variable as the value
            let cond_text = self.expr_to_string(&cond.condition);
            return Some((assign.lhs.clone(), cond_text));
        }

        None
    }

    fn arg_to_string(&self, arg: &gobject_ast::Argument) -> String {
        let gobject_ast::Argument::Expression(expr) = arg;
        self.expr_to_string(expr)
    }

    fn expr_to_string(&self, expr: &Expression) -> String {
        match expr {
            Expression::Identifier(id) => id.name.clone(),
            Expression::FieldAccess(f) => f.text.clone(),
            Expression::StringLiteral(s) => format!("\"{}\"", s.value),
            Expression::Unary(unary) => {
                // Handle *ptr, &ptr, etc.
                format!("{}{}", unary.operator.as_str(), self.expr_to_string(&unary.operand))
            }
            Expression::Call(call) => {
                // Reconstruct the call expression
                let args: Vec<String> = call
                    .arguments
                    .iter()
                    .map(|a| self.arg_to_string(a))
                    .collect();
                format!("{} ({})", call.function, args.join(", "))
            }
            _ => String::new(),
        }
    }
}
