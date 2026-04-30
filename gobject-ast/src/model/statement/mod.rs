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

use crate::model::{Argument, CallExpression, Expression, SourceLocation};

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

    /// Get all direct expressions contained in this statement (non-recursive)
    pub fn expressions(&self) -> Vec<&Expression> {
        match self {
            Statement::Expression(expr_stmt) => vec![&expr_stmt.expr],
            Statement::Return(ret) => ret.value.as_ref().into_iter().collect(),
            Statement::Declaration(decl) => decl.initializer.as_ref().into_iter().collect(),
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
            Statement::For(f) => &f.location,
            Statement::While(w) => &w.location,
            Statement::DoWhile(d) => &d.location,
            Statement::Break(b) => &b.location,
            Statement::Continue(c) => &c.location,
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
                for case in &switch.cases {
                    for stmt in &case.body {
                        stmt.walk_expressions(f);
                    }
                }
            }
            _ => {}
        }
    }

    /// Iterator over all switch statements in this statement tree (recursive)
    pub fn iter_switches(&self) -> impl Iterator<Item = &SwitchStatement> {
        Self::collect_switches(self).into_iter()
    }

    fn collect_switches(stmt: &Statement) -> Vec<&SwitchStatement> {
        let mut results = Vec::new();
        if let Statement::Switch(switch) = stmt {
            results.push(switch);
        }

        // Recurse into nested statements
        match stmt {
            Statement::If(if_stmt) => {
                for s in &if_stmt.then_body {
                    results.extend(Self::collect_switches(s));
                }
                if let Some(else_body) = &if_stmt.else_body {
                    for s in else_body {
                        results.extend(Self::collect_switches(s));
                    }
                }
            }
            Statement::Compound(compound) => {
                for s in &compound.statements {
                    results.extend(Self::collect_switches(s));
                }
            }
            Statement::Labeled(labeled) => {
                results.extend(Self::collect_switches(&labeled.statement));
            }
            Statement::Switch(switch) => {
                for case in &switch.cases {
                    for s in &case.body {
                        results.extend(Self::collect_switches(s));
                    }
                }
            }
            _ => {}
        }
        results
    }

    /// Iterator over all if statements in this statement tree (recursive)
    pub fn iter_if_statements(&self) -> impl Iterator<Item = &IfStatement> {
        Self::collect_if_statements(self).into_iter()
    }

    fn collect_if_statements(stmt: &Statement) -> Vec<&IfStatement> {
        let mut results = Vec::new();
        if let Statement::If(if_stmt) = stmt {
            results.push(if_stmt);
        }

        // Recurse into nested statements
        match stmt {
            Statement::If(if_stmt) => {
                for s in &if_stmt.then_body {
                    results.extend(Self::collect_if_statements(s));
                }
                if let Some(else_body) = &if_stmt.else_body {
                    for s in else_body {
                        results.extend(Self::collect_if_statements(s));
                    }
                }
            }
            Statement::Compound(compound) => {
                for s in &compound.statements {
                    results.extend(Self::collect_if_statements(s));
                }
            }
            Statement::Labeled(labeled) => {
                results.extend(Self::collect_if_statements(&labeled.statement));
            }
            Statement::Switch(switch) => {
                for case in &switch.cases {
                    for s in &case.body {
                        results.extend(Self::collect_if_statements(s));
                    }
                }
            }
            _ => {}
        }
        results
    }

    /// Iterator over all variable declarations in this statement tree
    /// (recursive)
    pub fn iter_declarations(&self) -> impl Iterator<Item = &VariableDecl> {
        Self::collect_declarations(self).into_iter()
    }

    fn collect_declarations(stmt: &Statement) -> Vec<&VariableDecl> {
        let mut results = Vec::new();
        if let Statement::Declaration(decl) = stmt {
            results.push(decl);
        }

        // Recurse into nested statements
        match stmt {
            Statement::If(if_stmt) => {
                for s in &if_stmt.then_body {
                    results.extend(Self::collect_declarations(s));
                }
                if let Some(else_body) = &if_stmt.else_body {
                    for s in else_body {
                        results.extend(Self::collect_declarations(s));
                    }
                }
            }
            Statement::Compound(compound) => {
                for s in &compound.statements {
                    results.extend(Self::collect_declarations(s));
                }
            }
            Statement::Labeled(labeled) => {
                results.extend(Self::collect_declarations(&labeled.statement));
            }
            Statement::Switch(switch) => {
                for case in &switch.cases {
                    for s in &case.body {
                        results.extend(Self::collect_declarations(s));
                    }
                }
            }
            _ => {}
        }
        results
    }

    /// Iterator over all return statements in this statement tree (recursive)
    pub fn iter_returns(&self) -> impl Iterator<Item = &ReturnStatement> {
        Self::collect_returns(self).into_iter()
    }

    fn collect_returns(stmt: &Statement) -> Vec<&ReturnStatement> {
        let mut results = Vec::new();
        if let Statement::Return(ret) = stmt {
            results.push(ret);
        }

        // Recurse into nested statements
        match stmt {
            Statement::If(if_stmt) => {
                for s in &if_stmt.then_body {
                    results.extend(Self::collect_returns(s));
                }
                if let Some(else_body) = &if_stmt.else_body {
                    for s in else_body {
                        results.extend(Self::collect_returns(s));
                    }
                }
            }
            Statement::Compound(compound) => {
                for s in &compound.statements {
                    results.extend(Self::collect_returns(s));
                }
            }
            Statement::Labeled(labeled) => {
                results.extend(Self::collect_returns(&labeled.statement));
            }
            Statement::Switch(switch) => {
                for case in &switch.cases {
                    for s in &case.body {
                        results.extend(Self::collect_returns(s));
                    }
                }
            }
            _ => {}
        }
        results
    }

    /// Iterator over all assignments in this statement tree (recursive)
    pub fn iter_assignments(&self) -> impl Iterator<Item = &crate::model::Assignment> {
        Self::collect_assignments(self).into_iter()
    }

    fn collect_assignments(stmt: &Statement) -> Vec<&crate::model::Assignment> {
        let mut results = Vec::new();
        if let Statement::Expression(expr_stmt) = stmt {
            if let Expression::Assignment(assignment) = &expr_stmt.expr {
                results.push(assignment);
            }
        }

        // Recurse into nested statements
        match stmt {
            Statement::If(if_stmt) => {
                for s in &if_stmt.then_body {
                    results.extend(Self::collect_assignments(s));
                }
                if let Some(else_body) = &if_stmt.else_body {
                    for s in else_body {
                        results.extend(Self::collect_assignments(s));
                    }
                }
            }
            Statement::Compound(compound) => {
                for s in &compound.statements {
                    results.extend(Self::collect_assignments(s));
                }
            }
            Statement::Labeled(labeled) => {
                results.extend(Self::collect_assignments(&labeled.statement));
            }
            Statement::Switch(switch) => {
                for case in &switch.cases {
                    for s in &case.body {
                        results.extend(Self::collect_assignments(s));
                    }
                }
            }
            _ => {}
        }
        results
    }

    /// Iterator over all call expressions in this statement tree (recursive)
    pub fn iter_calls(&self) -> impl Iterator<Item = &CallExpression> {
        Self::collect_calls(self).into_iter()
    }

    fn collect_calls(stmt: &Statement) -> Vec<&CallExpression> {
        let mut results = Vec::new();

        // Collect direct calls in this statement
        match stmt {
            Statement::Expression(expr_stmt) => {
                Self::collect_calls_from_expr(&expr_stmt.expr, &mut results);
            }
            Statement::Return(ret) => {
                if let Some(expr) = &ret.value {
                    Self::collect_calls_from_expr(expr, &mut results);
                }
            }
            Statement::Declaration(decl) => {
                if let Some(expr) = &decl.initializer {
                    Self::collect_calls_from_expr(expr, &mut results);
                }
            }
            _ => {}
        }

        // Recurse into nested statements
        match stmt {
            Statement::If(if_stmt) => {
                Self::collect_calls_from_expr(&if_stmt.condition, &mut results);
                for s in &if_stmt.then_body {
                    results.extend(Self::collect_calls(s));
                }
                if let Some(else_body) = &if_stmt.else_body {
                    for s in else_body {
                        results.extend(Self::collect_calls(s));
                    }
                }
            }
            Statement::Compound(compound) => {
                for s in &compound.statements {
                    results.extend(Self::collect_calls(s));
                }
            }
            Statement::Labeled(labeled) => {
                results.extend(Self::collect_calls(&labeled.statement));
            }
            Statement::Switch(switch) => {
                Self::collect_calls_from_expr(&switch.condition, &mut results);
                for case in &switch.cases {
                    for s in &case.body {
                        results.extend(Self::collect_calls(s));
                    }
                }
            }
            _ => {}
        }

        results
    }

    fn collect_calls_from_expr<'a>(expr: &'a Expression, results: &mut Vec<&'a CallExpression>) {
        match expr {
            Expression::Call(call) => {
                results.push(call);
                // Also check arguments for nested calls
                for arg in &call.arguments {
                    let Argument::Expression(arg_expr) = arg;
                    Self::collect_calls_from_expr(arg_expr, results);
                }
            }
            Expression::MacroCall(macro_call) => {
                for arg in &macro_call.arguments {
                    let Argument::Expression(arg_expr) = arg;
                    Self::collect_calls_from_expr(arg_expr, results);
                }
            }
            Expression::Assignment(assign) => {
                Self::collect_calls_from_expr(&assign.rhs, results);
            }
            Expression::Binary(binary) => {
                Self::collect_calls_from_expr(&binary.left, results);
                Self::collect_calls_from_expr(&binary.right, results);
            }
            Expression::Unary(unary) => {
                Self::collect_calls_from_expr(&unary.operand, results);
            }
            Expression::Cast(cast) => {
                Self::collect_calls_from_expr(&cast.operand, results);
            }
            Expression::Conditional(cond) => {
                Self::collect_calls_from_expr(&cond.condition, results);
                Self::collect_calls_from_expr(&cond.then_expr, results);
                Self::collect_calls_from_expr(&cond.else_expr, results);
            }
            Expression::Subscript(subscript) => {
                Self::collect_calls_from_expr(&subscript.array, results);
                Self::collect_calls_from_expr(&subscript.index, results);
            }
            Expression::Update(update) => {
                Self::collect_calls_from_expr(&update.operand, results);
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
