use serde::{Deserialize, Serialize};

use crate::model::{Expression, SourceLocation, UnaryOp};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnaryExpression {
    pub operator: UnaryOp,
    pub operand: Box<Expression>,
    pub location: SourceLocation,
}
