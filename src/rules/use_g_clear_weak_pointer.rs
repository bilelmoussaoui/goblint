use gobject_ast::{Expression, Statement, UnaryOp};

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGClearWeakPointer;

impl Rule for UseGClearWeakPointer {
    fn name(&self) -> &'static str {
        "use_g_clear_weak_pointer"
    }

    fn description(&self) -> &'static str {
        "Suggest g_clear_weak_pointer instead of manual g_object_remove_weak_pointer and NULL assignment"
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
            self.try_remove_weak_then_null(s1, s2, file, path, violations);
        });
    }
}

impl UseGClearWeakPointer {
    /// Matches `g_object_remove_weak_pointer(obj, &var); var = NULL;`
    fn try_remove_weak_then_null(
        &self,
        s1: &Statement,
        s2: &Statement,
        file: &gobject_ast::FileModel,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        // First statement must be g_object_remove_weak_pointer call
        let Statement::Expression(expr_stmt) = s1 else {
            return;
        };

        let Expression::Call(call) = &expr_stmt.expr else {
            return;
        };

        if !call.is_function("g_object_remove_weak_pointer") {
            return;
        }

        // Need at least 2 arguments
        if call.arguments.len() < 2 {
            return;
        }

        // Extract the variable from the second argument
        let Some(var_name) = self.extract_weak_pointer_var(&call.arguments[1]) else {
            return;
        };

        // Second statement must be var = NULL
        if !s2.is_null_assignment_to(&var_name) {
            return;
        }

        // Create a fix
        let replacement = format!("g_clear_weak_pointer (&{});", var_name);

        // Use two separate fixes to preserve comments between statements
        let fixes = vec![
            // Delete the entire first line
            Fix::delete_line(s1.location(), &file.source),
            // Replace the second statement
            Fix::new(
                s2.location().start_byte,
                s2.location().end_byte,
                replacement.clone(),
            ),
        ];

        violations.push(self.violation_with_fixes(
            file_path,
            s1.location().line,
            s1.location().column,
            format!(
                "Use {} instead of g_object_remove_weak_pointer + NULL assignment",
                replacement.trim_end_matches(';')
            ),
            fixes,
        ));
    }

    /// Extract variable name from the second argument of
    /// g_object_remove_weak_pointer Pattern: (gpointer*)&var or &var
    fn extract_weak_pointer_var(&self, arg: &gobject_ast::Argument) -> Option<String> {
        let gobject_ast::Argument::Expression(expr) = arg;

        // Handle cast expressions: (gpointer*)&var
        let inner_expr = if let Expression::Cast(cast) = expr.as_ref() {
            &cast.operand
        } else {
            expr
        };

        // Handle unary & operator: &var
        if let Expression::Unary(unary) = inner_expr.as_ref()
            && unary.operator == UnaryOp::AddressOf
        {
            return unary.operand.extract_variable_name();
        }

        None
    }
}
