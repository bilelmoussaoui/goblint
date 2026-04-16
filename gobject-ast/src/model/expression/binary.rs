use serde::{Deserialize, Serialize};

use crate::model::{Expression, SourceLocation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryExpression {
    pub left: Box<Expression>,
    pub operator: String,
    pub right: Box<Expression>,
    pub location: SourceLocation,
}

impl BinaryExpression {
    /// Check if this is a NULL comparison (x != NULL, x == NULL, etc.)
    pub fn is_null_check(&self) -> bool {
        (self.operator == "==" || self.operator == "!=")
            && (self.left.is_null() || self.right.is_null())
    }

    /// Extract the variable being compared in expressions like `x != 0`, `x >
    /// 0`, `0 < x`
    pub fn extract_compared_variable(&self) -> Option<String> {
        let left_is_zero = self.left.is_zero();
        let right_is_zero = self.right.is_zero();

        match self.operator.as_str() {
            "!=" | "==" | ">" | ">=" => {
                if right_is_zero {
                    self.left.extract_variable_name()
                } else if left_is_zero {
                    self.right.extract_variable_name()
                } else {
                    None
                }
            }
            "<" | "<=" => {
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
