use gobject_ast::{Expression, Statement};

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct GTaskSourceTag;

impl Rule for GTaskSourceTag {
    fn name(&self) -> &'static str {
        "g_task_source_tag"
    }

    fn description(&self) -> &'static str {
        "Ensure g_task_set_source_tag is called after g_task_new"
    }

    fn category(&self) -> super::Category {
        super::Category::Suspicious
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
        self.check_statements(path, func, &func.body_statements, source, violations);
    }
}

impl GTaskSourceTag {
    fn check_statements(
        &self,
        file_path: &std::path::Path,
        func: &gobject_ast::FunctionInfo,
        statements: &[Statement],
        source: &[u8],
        violations: &mut Vec<Violation>,
    ) {
        // Find all g_task_new calls and their variables
        let task_vars = self.find_gtask_new_vars(statements, source);

        // For each task variable, check if there's a set_source_tag call
        for (var_name, stmt_location) in task_vars {
            if !self.has_set_source_tag_call(statements, &var_name, source) {
                // Extract indentation from the statement
                let indentation = self.extract_indentation(stmt_location.start_byte, source);

                // Create fix: insert g_task_set_source_tag after the statement
                let fix = Fix::new(
                    stmt_location.end_byte,
                    stmt_location.end_byte,
                    format!(
                        "\n{}g_task_set_source_tag ({}, {});",
                        indentation, var_name, func.name
                    ),
                );

                violations.push(self.violation_with_fix(
                    file_path,
                    stmt_location.line,
                    stmt_location.column,
                    format!("GTask '{}' created without g_task_set_source_tag", var_name),
                    fix,
                ));
            }
        }
    }

    fn find_gtask_new_vars(
        &self,
        statements: &[Statement],
        source: &[u8],
    ) -> Vec<(String, gobject_ast::SourceLocation)> {
        let mut results = Vec::new();

        for stmt in statements {
            stmt.walk(&mut |s| {
                match s {
                    // Check declarations: GTask *task = g_task_new(...)
                    Statement::Declaration(decl) => {
                        if let Some(Expression::Call(call)) = &decl.initializer
                            && call.function == "g_task_new"
                        {
                            // Find the column where the variable name appears
                            let var_name_column = self.find_var_name_column(
                                decl.location.start_byte,
                                &decl.name,
                                source,
                            );
                            let mut location = decl.location.clone();
                            location.column = var_name_column;
                            results.push((decl.name.clone(), location));
                        }
                    }
                    // Check assignments: task = g_task_new(...)
                    Statement::Expression(expr_stmt) => {
                        if let Expression::Assignment(assignment) = &expr_stmt.expr
                            && let Expression::Call(call) = assignment.rhs.as_ref()
                            && call.function == "g_task_new"
                        {
                            // For assignments, use the assignment location
                            results.push((assignment.lhs.clone(), assignment.location.clone()));
                        }
                    }
                    _ => {}
                }
            });
        }

        results
    }

    fn find_var_name_column(&self, start_byte: usize, var_name: &str, source: &[u8]) -> usize {
        let search_text = std::str::from_utf8(&source[start_byte..]).unwrap_or("");
        if let Some(pos) = search_text.find(var_name) {
            let var_byte = start_byte + pos;

            // Find the start of the line
            let mut line_start = var_byte;
            while line_start > 0 && source[line_start - 1] != b'\n' {
                line_start -= 1;
            }

            // Return byte offset from line start (0-indexed becomes 1-indexed)
            var_byte - line_start
        } else {
            1
        }
    }

    fn has_set_source_tag_call(
        &self,
        statements: &[Statement],
        var_name: &str,
        source: &[u8],
    ) -> bool {
        for stmt in statements {
            let mut found = false;
            stmt.walk(&mut |s| {
                if let Some(call) = s.extract_call()
                    && call.function == "g_task_set_source_tag"
                    && !call.arguments.is_empty()
                {
                    // Check if first argument contains our variable
                    if let Some(arg_text) = call.get_arg_text(0, source)
                        && arg_text.contains(var_name)
                    {
                        found = true;
                    }
                }
            });
            if found {
                return true;
            }
        }
        false
    }

    fn extract_indentation(&self, start_byte: usize, source: &[u8]) -> String {
        // Find the start of the line
        let mut line_start_byte = start_byte;

        // Walk backwards to find the start of the line
        while line_start_byte > 0 && source[line_start_byte - 1] != b'\n' {
            line_start_byte -= 1;
        }

        // Count spaces/tabs from line start to first non-whitespace
        let mut indent = String::new();
        for &byte in &source[line_start_byte..start_byte] {
            if byte == b' ' || byte == b'\t' {
                indent.push(byte as char);
            } else {
                break;
            }
        }

        indent
    }
}
