mod break_stmt;
mod compound_stmt;
mod continue_stmt;
mod expression_stmt;
mod for_stmt;
mod goto_stmt;
mod if_stmt;
mod labeled_stmt;
mod return_stmt;
mod switch_stmt;
mod variable_decl;
mod while_stmt;

pub use break_stmt::BreakStatement;
pub use compound_stmt::CompoundStatement;
pub use continue_stmt::ContinueStatement;
pub use expression_stmt::ExpressionStmt;
pub use for_stmt::ForStatement;
pub use goto_stmt::GotoStatement;
pub use if_stmt::IfStatement;
pub use labeled_stmt::LabeledStatement;
pub use return_stmt::ReturnStatement;
use serde::{Deserialize, Serialize};
pub use switch_stmt::{CaseLabel, SwitchCase, SwitchStatement};
pub use variable_decl::VariableDecl;
pub use while_stmt::{DoWhileStatement, WhileStatement};

use super::top_level::PreprocessorDirective;
use crate::model::{CallExpression, Expression, SourceLocation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Statement {
    Declaration(VariableDecl),
    Expression(ExpressionStmt),
    If(IfStatement),
    Return(ReturnStatement),
    Goto(GotoStatement),
    Labeled(LabeledStatement),
    Compound(CompoundStatement),
    Switch(SwitchStatement),
    For(ForStatement),
    While(WhileStatement),
    DoWhile(DoWhileStatement),
    Break(BreakStatement),
    Continue(ContinueStatement),
    Preprocessor(PreprocessorDirective),
}

impl Statement {
    /// Recursively visit all nested statements. The closure receives a
    /// `&'s Statement` tied to `self`'s lifetime, so references extracted
    /// inside the closure can be stored in an outer `Vec<&'s T>`.
    pub fn walk<'s, F>(&'s self, f: &mut F)
    where
        F: FnMut(&'s Statement),
    {
        f(self);
        match self {
            Statement::If(if_stmt) => {
                for stmt in &if_stmt.then_body {
                    stmt.walk(f);
                }
                if let Some(else_body) = &if_stmt.else_body {
                    for stmt in else_body {
                        stmt.walk(f);
                    }
                }
            }
            Statement::Compound(compound) => {
                for stmt in &compound.statements {
                    stmt.walk(f);
                }
            }
            Statement::Labeled(labeled) => {
                labeled.statement.walk(f);
            }
            Statement::Switch(switch) => {
                for case in &switch.cases {
                    for stmt in &case.body {
                        stmt.walk(f);
                    }
                }
            }
            Statement::For(for_stmt) => {
                for stmt in &for_stmt.body {
                    stmt.walk(f);
                }
            }
            Statement::While(while_stmt) => {
                for stmt in &while_stmt.body {
                    stmt.walk(f);
                }
            }
            Statement::DoWhile(do_while) => {
                for stmt in &do_while.body {
                    stmt.walk(f);
                }
            }
            _ => {}
        }
    }

    /// Get all direct expressions contained in this statement (non-recursive).
    /// Includes conditions, initialisers, and all other expressions that are
    /// immediate children of this statement node.
    pub fn expressions(&self) -> Vec<&Expression> {
        match self {
            Statement::Expression(expr_stmt) => vec![&expr_stmt.expr],
            Statement::Return(ret) => ret.value.as_ref().into_iter().collect(),
            Statement::Declaration(decl) => decl.initializer.as_ref().into_iter().collect(),
            Statement::If(if_stmt) => vec![&if_stmt.condition],
            Statement::Switch(switch) => vec![&switch.condition],
            Statement::For(for_stmt) => {
                let mut exprs = Vec::new();
                if let Some(init) = &for_stmt.initializer {
                    exprs.push(&**init);
                }
                if let Some(cond) = &for_stmt.condition {
                    exprs.push(&**cond);
                }
                if let Some(update) = &for_stmt.update {
                    exprs.push(&**update);
                }
                exprs
            }
            Statement::While(while_stmt) => vec![&*while_stmt.condition],
            Statement::DoWhile(do_while) => vec![&*do_while.condition],
            Statement::Goto(_)
            | Statement::Labeled(_)
            | Statement::Compound(_)
            | Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Preprocessor(_) => vec![],
        }
    }

    pub fn location(&self) -> &SourceLocation {
        match self {
            Statement::Declaration(d) => &d.location,
            Statement::Expression(e) => &e.location,
            Statement::If(i) => &i.location,
            Statement::Return(r) => &r.location,
            Statement::Goto(g) => &g.location,
            Statement::Labeled(l) => &l.location,
            Statement::Compound(c) => &c.location,
            Statement::Switch(s) => &s.location,
            Statement::For(f) => &f.location,
            Statement::While(w) => &w.location,
            Statement::DoWhile(d) => &d.location,
            Statement::Break(b) => &b.location,
            Statement::Continue(c) => &c.location,
            Statement::Preprocessor(p) => p.location(),
        }
    }

    /// Recursively walk all expressions in this statement tree.
    /// Visits the direct expression of each statement (not sub-expressions —
    /// call `Expression::walk` on the result if you need nested expressions).
    pub fn walk_expressions<'s, F>(&'s self, f: &mut F)
    where
        F: FnMut(&'s Expression),
    {
        self.walk(&mut |s| {
            for expr in s.expressions() {
                f(expr);
            }
        });
    }

    /// Iterator over all switch statements in this statement tree (recursive)
    pub fn iter_switches<'s>(&'s self) -> impl Iterator<Item = &'s SwitchStatement> + 's {
        let mut results: Vec<&'s SwitchStatement> = Vec::new();
        self.walk(&mut |s| {
            if let Statement::Switch(sw) = s {
                results.push(sw);
            }
        });
        results.into_iter()
    }

    /// Iterator over all if statements in this statement tree (recursive)
    pub fn iter_if_statements<'s>(&'s self) -> impl Iterator<Item = &'s IfStatement> + 's {
        let mut results: Vec<&'s IfStatement> = Vec::new();
        self.walk(&mut |s| {
            if let Statement::If(if_stmt) = s {
                results.push(if_stmt);
            }
        });
        results.into_iter()
    }

    /// Iterator over all variable declarations in this statement tree
    /// (recursive)
    pub fn iter_declarations<'s>(&'s self) -> impl Iterator<Item = &'s VariableDecl> + 's {
        let mut results: Vec<&'s VariableDecl> = Vec::new();
        self.walk(&mut |s| {
            if let Statement::Declaration(decl) = s {
                results.push(decl);
            }
        });
        results.into_iter()
    }

    /// Iterator over all return statements in this statement tree (recursive)
    pub fn iter_returns<'s>(&'s self) -> impl Iterator<Item = &'s ReturnStatement> + 's {
        let mut results: Vec<&'s ReturnStatement> = Vec::new();
        self.walk(&mut |s| {
            if let Statement::Return(ret) = s {
                results.push(ret);
            }
        });
        results.into_iter()
    }

    /// Iterator over all top-level assignment statements in this statement tree
    /// (recursive). Only yields assignments that are the entire expression
    /// statement, not assignments nested inside other expressions.
    pub fn iter_assignments<'s>(
        &'s self,
    ) -> impl Iterator<Item = &'s crate::model::Assignment> + 's {
        let mut results: Vec<&'s crate::model::Assignment> = Vec::new();
        self.walk(&mut |s| {
            if let Statement::Expression(expr_stmt) = s {
                if let Expression::Assignment(assign) = &expr_stmt.expr {
                    results.push(assign);
                }
            }
        });
        results.into_iter()
    }

    /// Iterator over all call expressions in this statement tree (recursive).
    /// Includes calls at all nesting levels within expressions (e.g. nested
    /// arguments).
    pub fn iter_calls<'s>(&'s self) -> impl Iterator<Item = &'s CallExpression> + 's {
        // Two-step: first collect top-level expressions (avoids nested-closure
        // invariance issue with &mut Vec), then walk each for nested calls.
        let mut exprs: Vec<&'s Expression> = Vec::new();
        self.walk_expressions(&mut |expr| exprs.push(expr));

        let mut results: Vec<&'s CallExpression> = Vec::new();
        for expr in exprs {
            expr.walk(&mut |e| {
                if let Expression::Call(call) = e {
                    results.push(call);
                }
            });
        }
        results.into_iter()
    }

    /// Extract the call expression if this is an expression statement with a
    /// call
    pub fn extract_call(&self) -> Option<&CallExpression> {
        if let Statement::Expression(expr_stmt) = self {
            if let Expression::Call(call) = &expr_stmt.expr {
                return Some(call);
            }
        }
        None
    }

    /// Check if this statement assigns a value matching the predicate to the
    /// target variable
    pub fn is_assignment_to<F>(&self, target_var: &str, value_check: F) -> bool
    where
        F: Fn(&Expression) -> bool,
    {
        if let Statement::Expression(expr_stmt) = self {
            if let Expression::Assignment(assign) = &expr_stmt.expr {
                let lhs_text = match &*assign.lhs {
                    Expression::Identifier(id) => id.name.as_str(),
                    Expression::FieldAccess(field) => {
                        return field.text() == target_var && value_check(&assign.rhs);
                    }
                    _ => return false,
                };
                return lhs_text.trim() == target_var.trim() && value_check(&assign.rhs);
            }
        }
        false
    }

    /// Extract the assignment expression if this is an assignment statement
    pub fn extract_assignment(&self) -> Option<&crate::model::Assignment> {
        if let Statement::Expression(expr_stmt) = self {
            if let Expression::Assignment(assign) = &expr_stmt.expr {
                return Some(assign);
            }
        }
        None
    }

    /// Check if this statement assigns NULL to the target variable
    pub fn is_null_assignment_to(&self, var_name: &str) -> bool {
        self.is_assignment_to(var_name, |expr| expr.is_null())
    }

    /// Iterate over consecutive pairs of statements
    pub fn for_each_pair<F>(statements: &[Statement], mut f: F)
    where
        F: FnMut(&Statement, &Statement),
    {
        for i in 0..statements.len().saturating_sub(1) {
            f(&statements[i], &statements[i + 1]);
        }
    }

    /// Iterate over consecutive triples of statements
    pub fn for_each_triple<F>(statements: &[Statement], mut f: F)
    where
        F: FnMut(&Statement, &Statement, &Statement),
    {
        for i in 0..statements.len().saturating_sub(2) {
            f(&statements[i], &statements[i + 1], &statements[i + 2]);
        }
    }
}
