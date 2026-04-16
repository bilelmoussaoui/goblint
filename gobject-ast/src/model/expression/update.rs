use serde::{Deserialize, Serialize};

use crate::model::{Expression, SourceLocation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateExpression {
    pub operator: String, // "++", "--"
    pub operand: Box<Expression>,
    pub is_prefix: bool, // true for ++x, false for x++
    pub location: SourceLocation,
}
