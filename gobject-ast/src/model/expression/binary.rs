use serde::{Deserialize, Serialize};

use crate::model::{BinaryOp, Expression, SourceLocation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryExpression {
    pub left: Box<Expression>,
    pub operator: BinaryOp,
    pub right: Box<Expression>,
    pub location: SourceLocation,
}

impl BinaryExpression {
    /// Check if this is a NULL comparison (x != NULL, x == NULL, etc.)
    pub fn is_null_check(&self) -> bool {
        matches!(self.operator, BinaryOp::Equal | BinaryOp::NotEqual)
            && (self.left.is_null() || self.right.is_null())
    }

    /// Extract the variable being compared in expressions like `x != 0`, `x >
    /// 0`, `0 < x`, `x != NULL`, `NULL != x`
    pub fn extract_compared_variable(&self) -> Option<String> {
        let left_is_zero = self.left.is_zero();
        let right_is_zero = self.right.is_zero();
        let left_is_null = self.left.is_null();
        let right_is_null = self.right.is_null();

        match self.operator {
            BinaryOp::NotEqual | BinaryOp::Equal | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                if right_is_zero || right_is_null {
                    self.left.extract_variable_name()
                } else if left_is_zero || left_is_null {
                    self.right.extract_variable_name()
                } else {
                    None
                }
            }
            BinaryOp::Less | BinaryOp::LessEqual => {
                if left_is_zero {
                    self.right.extract_variable_name()
                } else if right_is_zero {
                    self.left.extract_variable_name()
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
