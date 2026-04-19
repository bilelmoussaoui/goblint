mod compound_stmt;
mod expression_stmt;
mod goto_stmt;
mod if_stmt;
mod labeled_stmt;
mod return_stmt;
mod switch_stmt;
mod variable_decl;

pub use compound_stmt::CompoundStatement;
pub use expression_stmt::ExpressionStmt;
pub use goto_stmt::GotoStatement;
pub use if_stmt::IfStatement;
pub use labeled_stmt::LabeledStatement;
pub use return_stmt::ReturnStatement;
use serde::{Deserialize, Serialize};
pub use switch_stmt::{CaseLabel, SwitchStatement};
pub use variable_decl::VariableDecl;

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
}

impl Statement {
    /// Recursively visit all nested statements
    pub fn walk<F>(&self, f: &mut F)
    where
        F: FnMut(&Statement),
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
                for stmt in &switch.body {
                    stmt.walk(f);
                }
            }
            _ => {}
        }
    }

    /// Get all direct expressions contained in this statement (non-recursive)
    pub fn expressions(&self) -> Vec<&Expression> {
        match self {
            Statement::Expression(expr_stmt) => vec![&expr_stmt.expr],
            Statement::Return(ret) => ret.value.as_ref().into_iter().collect(),
            Statement::Declaration(decl) => decl.initializer.as_ref().into_iter().collect(),
            _ => vec![],
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
        }
    }

    /// Recursively walk all expressions in this statement tree
    /// Visits each expression once, including nested expressions within other
    /// expressions
    pub fn walk_expressions<F>(&self, f: &mut F)
    where
        F: FnMut(&Expression),
    {
        // Visit direct expressions in this statement
        for expr in self.expressions() {
            f(expr);
        }

        // Recurse into nested statements
        match self {
            Statement::If(if_stmt) => {
                f(&if_stmt.condition);
                for stmt in &if_stmt.then_body {
                    stmt.walk_expressions(f);
                }
                if let Some(else_body) = &if_stmt.else_body {
                    for stmt in else_body {
                        stmt.walk_expressions(f);
                    }
                }
            }
            Statement::Compound(compound) => {
                for stmt in &compound.statements {
                    stmt.walk_expressions(f);
                }
            }
            Statement::Labeled(labeled) => {
                labeled.statement.walk_expressions(f);
            }
            Statement::Switch(switch) => {
                f(&switch.condition);
                for stmt in &switch.body {
                    stmt.walk_expressions(f);
                }
            }
            _ => {}
        }
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
                return assign.lhs.trim() == target_var.trim() && value_check(&assign.rhs);
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
