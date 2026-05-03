use gobject_ast::{AssignmentOp, Expression, Statement, UnaryOp};

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGSetObject;

impl Rule for UseGSetObject {
    fn name(&self) -> &'static str {
        "use_g_set_object"
    }

    fn description(&self) -> &'static str {
        "Suggest g_set_object() instead of manual g_clear_object and g_object_ref"
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
            self.try_clear_then_ref(s1, s2, file, path, violations);
        });
    }
}

impl UseGSetObject {
    /// Check for g_clear_object(&var)/g_object_unref(var) followed by var =
    /// g_object_ref(...)
    fn try_clear_then_ref(
        &self,
        s1: &Statement,
        s2: &Statement,
        file: &gobject_ast::FileModel,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) -> bool {
        // First statement: g_clear_object(&var) or g_object_unref(var)
        let Some((var_name, needs_deref)) = self.extract_clear_or_unref_var(s1) else {
            return false;
        };

        // Second statement: var = g_object_ref(...) or *var = g_object_ref(...)
        let Some((assign_var, new_val)) = self.extract_object_ref_assignment(s2) else {
            return false;
        };

        // Check if variables match (accounting for * dereference)
        let expected_assign = if needs_deref {
            format!("*{}", var_name)
        } else {
            var_name.clone()
        };

        if assign_var != expected_assign {
            return false;
        }

        // g_set_object takes GObject**, so:
        // - If var is GObject* (needs_deref=false), use &var
        // - If var is GObject** (needs_deref=true), use var directly
        let set_object_call = if needs_deref {
            format!("g_set_object ({var_name}, {new_val});")
        } else {
            format!("g_set_object (&{var_name}, {new_val});")
        };

        // Use two separate fixes to preserve comments between statements
        let fixes = vec![
            // Delete the entire first line (g_clear_object/g_object_unref)
            Fix::delete_line(s1.location(), &file.source),
            // Replace the second statement with g_set_object
            Fix::new(
                s2.location().start_byte,
                s2.location().end_byte,
                set_object_call.clone(),
            ),
        ];

        violations.push(self.violation_with_fixes(
            file_path,
            s1.location().line,
            s1.location().column,
            format!("Use {set_object_call} instead of g_clear_object and g_object_ref"),
            fixes,
        ));
        true
    }

    /// Extract variable from g_clear_object(&var)/g_clear_object(ptr) or
    /// g_object_unref(var) Returns (var_name, needs_deref) where
    /// needs_deref indicates if assignment should use *var
    fn extract_clear_or_unref_var(&self, stmt: &Statement) -> Option<(String, bool)> {
        let Statement::Expression(expr_stmt) = stmt else {
            return None;
        };

        let Expression::Call(call) = &expr_stmt.expr else {
            return None;
        };

        if call.arguments.is_empty() {
            return None;
        }

        if call.is_function("g_clear_object") {
            // g_clear_object can take:
            // 1. &var - then assignment is var = ...
            // 2. ptr - then assignment is *ptr = ...
            let first_arg = call.get_arg(0)?;
            if let Expression::Unary(unary) = first_arg
                && unary.operator == UnaryOp::AddressOf
            {
                // Case 1: g_clear_object(&var)
                return Some((unary.operand.to_text(), false));
            } else {
                // Case 2: g_clear_object(ptr) where ptr is GObject**
                return Some((first_arg.to_text(), true));
            }
        } else if call.is_function("g_object_unref") {
            // g_object_unref(var) - assignment is var = ...
            let first_arg = call.get_arg(0)?;
            return Some((first_arg.to_text(), false));
        }

        None
    }

    /// Extract (var, new_val) from var = g_object_ref(new_val)
    fn extract_object_ref_assignment(&self, stmt: &Statement) -> Option<(String, String)> {
        let Statement::Expression(expr_stmt) = stmt else {
            return None;
        };

        let Expression::Assignment(assign) = &expr_stmt.expr else {
            return None;
        };

        if assign.operator != AssignmentOp::Assign {
            return None;
        }

        // var = g_object_ref(new_val)
        if let Expression::Call(call) = &*assign.rhs
            && call.is_function("g_object_ref")
            && !call.arguments.is_empty()
        {
            let new_val = call.get_arg(0)?.to_text();
            let var_name = assign.lhs_as_text();
            if !var_name.is_empty() {
                return Some((var_name, new_val));
            }
        }

        None
    }
}
