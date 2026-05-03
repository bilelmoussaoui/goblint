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
        func: &gobject_ast::top_level::FunctionDefItem,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        let file = ast_context.project.files.get(path).unwrap();
        Statement::walk_pairs(&func.body_statements, &mut |s1, s2| {
            self.try_free_then_strdup(s1, s2, file, path, violations);
        });
    }
}

impl UseGSetStr {
    /// Check for g_free(var) followed by var = g_strdup(...)
    fn try_free_then_strdup(
        &self,
        s1: &Statement,
        s2: &Statement,
        file: &gobject_ast::FileModel,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) -> bool {
        // First statement: g_free(var) or g_clear_pointer(&var, g_free)
        let Some(var_name) = self.extract_gfree_var(s1, &file.source) else {
            return false;
        };

        // Second statement: var = g_strdup(...)
        let Some((assign_var, new_val)) = self.extract_strdup_assignment(s2, &file.source) else {
            return false;
        };

        if assign_var != var_name {
            return false;
        }

        let set_str_call = format!("g_set_str (&{var_name}, {new_val});");

        // Use two separate fixes to preserve comments between statements
        let fixes = vec![
            // Delete the entire first line (g_free/g_clear_pointer)
            Fix::delete_line(s1.location(), &file.source),
            // Replace the second statement with g_set_str
            Fix::new(
                s2.location().start_byte,
                s2.location().end_byte,
                set_str_call.clone(),
            ),
        ];

        violations.push(self.violation_with_fixes(
            file_path,
            s1.location().line,
            s1.location().column,
            format!("Use {set_str_call} instead of g_free and g_strdup"),
            fixes,
        ));
        true
    }

    /// Extract variable from g_free(var) or g_clear_pointer(&var, g_free)
    fn extract_gfree_var(&self, stmt: &Statement, source: &[u8]) -> Option<String> {
        let Statement::Expression(expr_stmt) = stmt else {
            return None;
        };

        let Expression::Call(call) = &expr_stmt.expr else {
            return None;
        };

        if call.is_function("g_free") {
            return call.get_arg(0)?.to_source_string(source);
        } else if call.is_function("g_clear_pointer") {
            // g_clear_pointer(&var, g_free)
            if call.arguments.len() != 2 {
                return None;
            }

            // Check if second argument is g_free
            let second_arg = call.get_arg(1)?;

            if let Expression::Identifier(id) = second_arg {
                if id.name != "g_free" {
                    return None;
                }
            } else {
                return None;
            }

            // First argument is &var - extract var
            let first_arg = call.get_arg(0)?;
            if let Expression::Unary(unary) = first_arg
                && unary.operator == UnaryOp::AddressOf
            {
                return unary.operand.to_source_string(source);
            }
        }

        None
    }

    /// Extract (var, new_val) from var = g_strdup(new_val) or var = cond ?
    /// g_strdup(...) : NULL
    fn extract_strdup_assignment(
        &self,
        stmt: &Statement,
        source: &[u8],
    ) -> Option<(String, String)> {
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
            && call.is_function("g_strdup")
            && !call.arguments.is_empty()
        {
            let new_val = call.get_arg(0)?.to_source_string(source)?;
            let var_name = assign.lhs_as_text();
            if !var_name.is_empty() {
                return Some((var_name, new_val));
            }
        }

        // Ternary: var = cond ? g_strdup(...) : NULL
        if let Expression::Conditional(cond) = &*assign.rhs
            && cond.then_expr.is_call_to_any(&["g_strdup", "g_strndup"])
        {
            // Use the condition variable as the value
            let cond_text = cond.condition.to_source_string(source)?;
            let var_name = assign.lhs_as_text();
            if !var_name.is_empty() {
                return Some((var_name, cond_text));
            }
        }

        None
    }
}
