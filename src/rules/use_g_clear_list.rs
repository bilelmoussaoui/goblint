use gobject_ast::Statement;

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGClearList;

impl Rule for UseGClearList {
    fn name(&self) -> &'static str {
        "use_g_clear_list"
    }

    fn description(&self) -> &'static str {
        "Suggest g_clear_list/g_clear_slist instead of manual g_list_free/g_slist_free and NULL assignment"
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

        let source = &ast_context.project.files.get(path).unwrap().source;
        self.check_statements(path, &func.body_statements, source, violations);
    }
}

impl UseGClearList {
    fn check_statements(
        &self,
        file_path: &std::path::Path,
        statements: &[Statement],
        source: &[u8],
        violations: &mut Vec<Violation>,
    ) {
        // Check consecutive statements for the pattern
        self.check_free_then_null(file_path, statements, source, violations);

        // Recurse into nested statements
        for stmt in statements {
            match stmt {
                Statement::If(if_stmt) => {
                    self.check_statements(file_path, &if_stmt.then_body, source, violations);
                    if let Some(else_body) = &if_stmt.else_body {
                        self.check_statements(file_path, else_body, source, violations);
                    }
                }
                Statement::Compound(compound) => {
                    self.check_statements(file_path, &compound.statements, source, violations);
                }
                _ => {}
            }
        }
    }

    fn check_free_then_null(
        &self,
        file_path: &std::path::Path,
        statements: &[Statement],
        source: &[u8],
        violations: &mut Vec<Violation>,
    ) {
        for i in 0..statements.len().saturating_sub(1) {
            let first = &statements[i];
            let second = &statements[i + 1];

            // Check if first is g_list_free or g_slist_free
            if let Some((var_name, list_type)) = self.extract_list_free(first, source) {
                // Check if second is assignment to NULL
                if let Some(assign_var) = self.extract_null_assignment(second)
                    && assign_var.trim() == var_name.trim()
                {
                    let clear_fn = if list_type == "GList" {
                        "g_clear_list"
                    } else {
                        "g_clear_slist"
                    };

                    let replacement = format!("{} (&{}, NULL);", clear_fn, var_name);

                    let fix = Fix::new(
                        first.location().start_byte,
                        second.location().end_byte,
                        replacement.clone(),
                    );

                    violations.push(self.violation_with_fix(
                        file_path,
                        first.location().line,
                        first.location().column,
                        format!(
                            "Use {} instead of {}_free and NULL assignment",
                            replacement,
                            list_type.to_lowercase()
                        ),
                        fix,
                    ));
                }
            }
        }
    }

    fn extract_list_free(&self, stmt: &Statement, source: &[u8]) -> Option<(String, &'static str)> {
        let call = stmt.extract_call()?;

        let list_type = match call.function.as_str() {
            "g_list_free" => "GList",
            "g_slist_free" => "GSList",
            _ => return None,
        };

        if call.arguments.is_empty() {
            return None;
        }

        let var_name = call.get_arg_text(0, source)?;
        Some((var_name, list_type))
    }

    fn extract_null_assignment(&self, stmt: &Statement) -> Option<String> {
        if let Statement::Expression(expr_stmt) = stmt
            && let gobject_ast::Expression::Assignment(assignment) = &expr_stmt.expr
            && (assignment.rhs.is_null() || assignment.rhs.is_zero())
        {
            Some(assignment.lhs.clone())
        } else {
            None
        }
    }
}
