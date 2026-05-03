use gobject_ast::{AssignmentOp, BinaryOp, Expression, Statement};

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGStealPointer;

impl Rule for UseGStealPointer {
    fn name(&self) -> &'static str {
        "use_g_steal_pointer"
    }

    fn description(&self) -> &'static str {
        "Use g_steal_pointer() instead of manually copying a pointer and setting it to NULL"
    }

    fn category(&self) -> super::Category {
        super::Category::Style
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
        self.check_function(func, path, file, violations);
    }
}

impl UseGStealPointer {
    fn check_function(
        &self,
        func: &gobject_ast::top_level::FunctionDefItem,
        file_path: &std::path::Path,
        file: &gobject_ast::FileModel,
        violations: &mut Vec<Violation>,
    ) {
        self.check_statements(&func.body_statements, file_path, file, violations);
    }

    fn check_statements(
        &self,
        statements: &[Statement],
        file_path: &std::path::Path,
        file: &gobject_ast::FileModel,
        violations: &mut Vec<Violation>,
    ) {
        let mut i = 0;
        while i < statements.len() {
            if self.try_if_else_steal(&statements[i], file_path, &file.source, violations) {
                i += 1;
                continue;
            }
            if self.try_if_no_else_steal(&statements[i], file_path, &file.source, violations) {
                i += 1;
                continue;
            }
            if i + 2 < statements.len()
                && self.try_declare_null_return(
                    &statements[i],
                    &statements[i + 1],
                    &statements[i + 2],
                    file,
                    file_path,
                    violations,
                )
            {
                i += 3;
                continue;
            }
            if i + 1 < statements.len()
                && self.try_assign_null(
                    &statements[i],
                    &statements[i + 1],
                    file,
                    file_path,
                    violations,
                )
            {
                i += 2;
                continue;
            }
            match &statements[i] {
                Statement::Compound(compound) => {
                    self.check_statements(&compound.statements, file_path, file, violations);
                }
                Statement::If(if_stmt) => {
                    self.check_statements(&if_stmt.then_body, file_path, file, violations);
                    if let Some(else_body) = &if_stmt.else_body {
                        self.check_statements(else_body, file_path, file, violations);
                    }
                }
                Statement::Labeled(labeled) => {
                    self.check_statements(
                        std::slice::from_ref(&labeled.statement),
                        file_path,
                        file,
                        violations,
                    );
                }
                _ => {}
            }
            i += 1;
        }
    }

    /// Matches: `T *tmp = ptr_expr; ptr_expr = NULL; return tmp;`
    fn try_declare_null_return(
        &self,
        s1: &Statement,
        s2: &Statement,
        s3: &Statement,
        file: &gobject_ast::FileModel,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) -> bool {
        // s1: T *tmp = ptr_expr
        let Statement::Declaration(decl) = s1 else {
            return false;
        };

        let Some(init_expr) = &decl.initializer else {
            return false;
        };

        // Skip NULL initializers
        if init_expr.is_null() {
            return false;
        }

        // Get the variable name from the initializer
        let Some(ptr_expr) = init_expr.extract_variable_name() else {
            return false;
        };

        // Skip dereferences
        if ptr_expr.starts_with('*') {
            return false;
        }

        let tmp_name = &decl.name;

        // s2: ptr_expr = NULL
        if !s2.is_null_assignment_to(&ptr_expr) {
            return false;
        }

        // s3: return tmp
        let Statement::Return(ret) = s3 else {
            return false;
        };

        if let Some(Expression::Identifier(id)) = &ret.value {
            if id.name != *tmp_name {
                return false;
            }
        } else {
            return false;
        }

        let replacement = format!("return g_steal_pointer (&{ptr_expr});");

        // Use three separate fixes to preserve comments between statements
        let fixes = vec![
            // Delete the first two lines
            Fix::delete_line(s1.location(), &file.source),
            Fix::delete_line(s2.location(), &file.source),
            // Replace the third statement (return)
            Fix::new(
                s3.location().start_byte,
                s3.location().end_byte,
                replacement.clone(),
            ),
        ];

        violations.push(self.violation_with_fixes(
            file_path,
            s1.location().line,
            s1.location().column,
            format!("Use {replacement} instead of copying {ptr_expr} and setting it to NULL"),
            fixes,
        ));
        true
    }

    /// Matches: `other_expr = ptr_expr; ptr_expr = NULL;`
    fn try_assign_null(
        &self,
        s1: &Statement,
        s2: &Statement,
        file: &gobject_ast::FileModel,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) -> bool {
        let Some((other_expr, ptr_expr)) = self.extract_assignment(s1) else {
            return false;
        };

        // Skip dereference expressions — g_steal_pointer (&*expr) is confusing
        if ptr_expr.starts_with('*') {
            return false;
        }

        if !s2.is_null_assignment_to(&ptr_expr) {
            return false;
        }

        let replacement = format!("{other_expr} = g_steal_pointer (&{ptr_expr});");

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
            format!("Use g_steal_pointer (&{ptr_expr}) instead of copying and setting to NULL"),
            fixes,
        ));
        true
    }

    /// Matches: if (expr) { dest = expr; expr = NULL; } else { dest = NULL; }
    fn try_if_else_steal(
        &self,
        stmt: &Statement,
        file_path: &std::path::Path,
        _source: &[u8],
        violations: &mut Vec<Violation>,
    ) -> bool {
        let Statement::If(if_stmt) = stmt else {
            return false;
        };

        // Must have else block
        let Some(else_body) = &if_stmt.else_body else {
            return false;
        };

        // Extract tested expression from condition
        let Some(expr_text) = self.extract_condition_expr(&if_stmt.condition) else {
            return false;
        };

        // Skip dereference expressions
        if expr_text.starts_with('*') {
            return false;
        }

        // Then-block must have exactly 2 statements
        if if_stmt.then_body.len() != 2 {
            return false;
        }

        // then_body[0]: dest = expr
        let Some((dest_expr, rhs)) = self.extract_assignment(&if_stmt.then_body[0]) else {
            return false;
        };
        if rhs != expr_text {
            return false;
        }

        // then_body[1]: expr = NULL
        if !if_stmt.then_body[1].is_null_assignment_to(&expr_text) {
            return false;
        }

        // Else-block must have exactly 1 statement: dest = NULL
        if else_body.len() != 1 {
            return false;
        }
        if !else_body[0].is_null_assignment_to(&dest_expr) {
            return false;
        }

        let replacement = format!("{dest_expr} = g_steal_pointer (&{expr_text});");
        let fix = Fix::new(
            if_stmt.location.start_byte,
            if_stmt.location.end_byte,
            replacement.clone(),
        );
        violations.push(self.violation_with_fix(
            file_path,
            if_stmt.location.line,
            if_stmt.location.column,
            format!("Use g_steal_pointer (&{expr_text}) instead of if/else copy-and-NULL pattern"),
            fix,
        ));
        true
    }

    /// Extract the tested pointer expression from an if-condition
    /// Handles bare `expr`, `expr != NULL`, and `NULL != expr`
    fn extract_condition_expr(&self, condition: &Expression) -> Option<String> {
        match condition {
            Expression::Identifier(id) => Some(id.name.clone()),
            Expression::FieldAccess(f) => Some(f.text()),
            Expression::Binary(bin) => {
                if bin.operator == BinaryOp::NotEqual {
                    // Check for expr != NULL or NULL != expr
                    if matches!(&*bin.right, Expression::Null(_)) {
                        // expr != NULL, return left side
                        return self.extract_simple_expr(&bin.left);
                    }
                    if matches!(&*bin.left, Expression::Null(_)) {
                        // NULL != expr, return right side
                        return self.extract_simple_expr(&bin.right);
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn extract_simple_expr(&self, expr: &Expression) -> Option<String> {
        match expr {
            Expression::Identifier(id) => Some(id.name.clone()),
            Expression::FieldAccess(f) => Some(f.text()),
            _ => None,
        }
    }

    /// Matches if-without-else with steal pattern in body
    /// if (c) { dest = ptr; ptr = NULL; } or if (c) { T *tmp = ptr; ptr = NULL;
    /// return tmp; }
    fn try_if_no_else_steal(
        &self,
        stmt: &Statement,
        file_path: &std::path::Path,
        source: &[u8],
        violations: &mut Vec<Violation>,
    ) -> bool {
        let Statement::If(if_stmt) = stmt else {
            return false;
        };

        // Must have no else
        if if_stmt.else_body.is_some() {
            return false;
        }

        // Try to extract condition expression
        let condition_expr = self.extract_condition_expr(&if_stmt.condition);

        // Pattern 1: 2 statements - dest = ptr; ptr = NULL;
        if if_stmt.then_body.len() == 2 {
            let Some((dest_expr, ptr_expr)) = self.extract_assignment(&if_stmt.then_body[0]) else {
                return false;
            };

            // Skip dereference expressions
            if ptr_expr.starts_with('*') {
                return false;
            }

            if !if_stmt.then_body[1].is_null_assignment_to(&ptr_expr) {
                return false;
            }

            let replacement = format!("{dest_expr} = g_steal_pointer (&{ptr_expr});");

            // If condition tests the same variable being stolen, remove entire if
            // Otherwise just replace the body
            let fix = if condition_expr.as_ref() == Some(&ptr_expr) {
                Fix::new(
                    if_stmt.location.start_byte,
                    if_stmt.location.end_byte,
                    replacement.clone(),
                )
            } else if if_stmt.then_has_braces {
                // If it has braces, remove them and replace body with single statement
                let body_start = if_stmt.then_body[0].location().start_byte;
                let (open_brace, close_brace) =
                    gobject_ast::SourceLocation::find_braces_around(body_start, source);
                // The `{` is on its own line with indentation already in the source.
                // When we replace from `{` to `}`, that indentation before `{` stays in place.
                // So we don't add any extra indentation to the replacement.
                Fix::new(open_brace, close_brace, replacement.clone())
            } else {
                // No braces, just replace the body
                let body_start = if_stmt.then_body[0].location().start_byte;
                let body_end = if_stmt.then_body[1].location().end_byte;
                Fix::new(body_start, body_end, replacement.clone())
            };

            violations.push(self.violation_with_fix(
                file_path,
                if_stmt.then_body[0].location().line,
                if_stmt.then_body[0].location().column,
                format!("Use g_steal_pointer (&{ptr_expr}) instead of copying and setting to NULL"),
                fix,
            ));
            return true;
        }

        // Pattern 2: 3 statements - T *tmp = ptr; ptr = NULL; return tmp;
        if if_stmt.then_body.len() == 3 {
            let Statement::Declaration(decl) = &if_stmt.then_body[0] else {
                return false;
            };

            let Some(init_expr) = &decl.initializer else {
                return false;
            };

            // Skip NULL initializers
            if init_expr.is_null() {
                return false;
            }

            let Some(ptr_expr) = init_expr.extract_variable_name() else {
                return false;
            };

            // Skip dereference expressions
            if ptr_expr.starts_with('*') {
                return false;
            }

            let tmp_name = &decl.name;

            if !if_stmt.then_body[1].is_null_assignment_to(&ptr_expr) {
                return false;
            }

            // Third statement must be return tmp
            let Statement::Return(ret) = &if_stmt.then_body[2] else {
                return false;
            };

            if let Some(Expression::Identifier(id)) = &ret.value {
                if id.name != *tmp_name {
                    return false;
                }
            } else {
                return false;
            }

            let replacement = format!("return g_steal_pointer (&{ptr_expr});");

            // If condition tests the same variable being stolen, remove entire if
            let fix = if condition_expr.as_ref() == Some(&ptr_expr) {
                Fix::new(
                    if_stmt.location.start_byte,
                    if_stmt.location.end_byte,
                    replacement.clone(),
                )
            } else if if_stmt.then_has_braces {
                // If it has braces, remove them and replace body with single statement
                let body_start = if_stmt.then_body[0].location().start_byte;
                let (open_brace, close_brace) =
                    gobject_ast::SourceLocation::find_braces_around(body_start, source);
                // The `{` is on its own line with indentation already in the source.
                // When we replace from `{` to `}`, that indentation before `{` stays in place.
                // So we don't add any extra indentation to the replacement.
                Fix::new(open_brace, close_brace, replacement.clone())
            } else {
                // No braces, just replace the body
                let body_start = if_stmt.then_body[0].location().start_byte;
                let body_end = if_stmt.then_body[2].location().end_byte;
                Fix::new(body_start, body_end, replacement.clone())
            };

            violations.push(self.violation_with_fix(
                file_path,
                if_stmt.then_body[0].location().line,
                if_stmt.then_body[0].location().column,
                format!("Use {replacement} instead of copying {ptr_expr} and setting it to NULL"),
                fix,
            ));
            return true;
        }

        false
    }

    /// Extract (lhs, rhs) from assignment statement
    fn extract_assignment(&self, stmt: &Statement) -> Option<(String, String)> {
        let Statement::Expression(expr_stmt) = stmt else {
            return None;
        };

        let Expression::Assignment(assign) = &expr_stmt.expr else {
            return None;
        };

        if assign.operator != AssignmentOp::Assign {
            return None;
        }

        // Get rhs as string - handle various expression types
        let rhs = match &*assign.rhs {
            Expression::Identifier(id) => id.name.clone(),
            Expression::FieldAccess(f) => f.text(),
            Expression::Null(_) | Expression::Call(_) => {
                // For NULL or function calls like g_strdup(), we don't want to suggest
                // g_steal_pointer
                return None;
            }
            _ => {
                return None;
            }
        };

        let lhs = assign.lhs_as_text();
        if lhs.is_empty() {
            return None;
        }
        Some((lhs, rhs))
    }
}
