use serde::{Deserialize, Serialize};

use crate::model::statement::Statement;
use crate::model::expression::{CallExpression, Expression, Argument};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub name: String,
    pub line: usize,
    pub is_static: bool,
    pub export_macros: Vec<String>, // CLUTTER_EXPORT, G_MODULE_EXPORT, G_DEPRECATED_FOR, etc.
    pub has_static_forward_decl: bool, // Has a static forward declaration in the same file
    pub is_definition: bool,        // true = definition, false = declaration
    pub return_type: Option<String>,
    pub parameters: Vec<Parameter>,
    /// Byte range of the entire function (for definitions) - use with
    /// FileModel.source
    pub start_byte: Option<usize>,
    pub end_byte: Option<usize>,
    /// Byte range of just the function body (for definitions) - use with
    /// FileModel.source
    pub body_start_byte: Option<usize>,
    pub body_end_byte: Option<usize>,
    /// Parsed body statements (for definitions) - ordered list
    pub body_statements: Vec<Statement>,
}

impl FunctionInfo {
    /// Find all calls to specific functions in the body
    /// Returns references to all CallExpression nodes that match any of the
    /// given function names
    pub fn find_calls<'a>(&'a self, function_names: &[&str]) -> Vec<&'a CallExpression> {
        let mut calls = Vec::new();
        self.find_calls_recursive(&self.body_statements, function_names, &mut calls);
        calls
    }

    fn find_calls_recursive<'a>(
        &'a self,
        statements: &'a [Statement],
        function_names: &[&str],
        calls: &mut Vec<&'a CallExpression>,
    ) {
        for stmt in statements {
            match stmt {
                Statement::Expression(expr_stmt) => {
                    self.find_calls_in_expr(&expr_stmt.expr, function_names, calls);
                }
                Statement::Return(ret) => {
                    if let Some(expr) = &ret.value {
                        self.find_calls_in_expr(expr, function_names, calls);
                    }
                }
                Statement::Declaration(decl) => {
                    if let Some(expr) = &decl.initializer {
                        self.find_calls_in_expr(expr, function_names, calls);
                    }
                }
                Statement::If(if_stmt) => {
                    self.find_calls_in_expr(&if_stmt.condition, function_names, calls);
                    self.find_calls_recursive(&if_stmt.then_body, function_names, calls);
                    if let Some(else_body) = &if_stmt.else_body {
                        self.find_calls_recursive(else_body, function_names, calls);
                    }
                }
                Statement::Compound(compound) => {
                    self.find_calls_recursive(&compound.statements, function_names, calls);
                }
                Statement::Labeled(labeled) => {
                    self.find_calls_recursive(
                        std::slice::from_ref(&labeled.statement),
                        function_names,
                        calls,
                    );
                }
                _ => {}
            }
        }
    }

    fn find_calls_in_expr<'a>(
        &'a self,
        expr: &'a Expression,
        function_names: &[&str],
        calls: &mut Vec<&'a CallExpression>,
    ) {
        match expr {
            Expression::Call(call) => {
                if function_names.contains(&call.function.as_str()) {
                    calls.push(call);
                }
                // Also check arguments
                for arg in &call.arguments {
                    let Argument::Expression(e) = arg;
                    self.find_calls_in_expr(e, function_names, calls);
                }
            }
            Expression::Assignment(assign) => {
                self.find_calls_in_expr(&assign.rhs, function_names, calls);
            }
            Expression::Binary(binary) => {
                self.find_calls_in_expr(&binary.left, function_names, calls);
                self.find_calls_in_expr(&binary.right, function_names, calls);
            }
            Expression::Unary(unary) => {
                self.find_calls_in_expr(&unary.operand, function_names, calls);
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: Option<String>,
    pub type_name: String,
}
