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
        func: &gobject_ast::top_level::FunctionDefItem,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        let file = ast_context.project.files.get(path).unwrap();
        self.check_statements(path, &func.body_statements, file, violations);
    }
}

impl UseGClearList {
    fn check_statements(
        &self,
        file_path: &std::path::Path,
        statements: &[Statement],
        file: &gobject_ast::FileModel,
        violations: &mut Vec<Violation>,
    ) {
        // Check consecutive statements for the pattern
        self.check_free_then_null(file_path, statements, file, violations);

        // Recurse into nested statements
        for stmt in statements {
            match stmt {
                Statement::If(if_stmt) => {
                    self.check_statements(file_path, &if_stmt.then_body, file, violations);
                    if let Some(else_body) = &if_stmt.else_body {
                        self.check_statements(file_path, else_body, file, violations);
                    }
                }
                Statement::Compound(compound) => {
                    self.check_statements(file_path, &compound.statements, file, violations);
                }
                _ => {}
            }
        }
    }

    fn check_free_then_null(
        &self,
        file_path: &std::path::Path,
        statements: &[Statement],
        file: &gobject_ast::FileModel,
        violations: &mut Vec<Violation>,
    ) {
        Statement::for_each_pair(statements, |first, second| {
            // Check if first is g_list_free or g_slist_free
            if let Some((var_name, list_type)) = self.extract_list_free(first, &file.source) {
                // Check if second is assignment to NULL
                if second.is_null_assignment_to(&var_name) {
                    let clear_fn = if list_type == "GList" {
                        "g_clear_list"
                    } else {
                        "g_clear_slist"
                    };

                    let replacement = format!("{} (&{}, NULL);", clear_fn, var_name);

                    // Use two separate fixes to preserve comments between statements
                    let fixes = vec![
                        // Delete the entire first line
                        Fix::delete_line(first.location(), &file.source),
                        // Replace the second statement
                        Fix::new(
                            second.location().start_byte,
                            second.location().end_byte,
                            replacement.clone(),
                        ),
                    ];

                    let base_type = match list_type {
                        "GList" => "g_list",
                        "GSList" => "g_slist",
                        _ => unreachable!(),
                    };

                    violations.push(self.violation_with_fixes(
                        file_path,
                        first.location().line,
                        first.location().column,
                        format!(
                            "Use {replacement} instead of {base_type}_free and NULL assignment",
                        ),
                        fixes,
                    ));
                }
            }
        });
    }

    fn extract_list_free(&self, stmt: &Statement, source: &[u8]) -> Option<(String, &'static str)> {
        let call = stmt.extract_call()?;

        let func_name = call.function_name_str()?;
        let list_type = match func_name {
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
}
