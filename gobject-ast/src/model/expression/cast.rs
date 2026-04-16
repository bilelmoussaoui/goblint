use serde::{Deserialize, Serialize};

use crate::model::{Expression, SourceLocation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastExpression {
    pub type_name: String,
    pub operand: Box<Expression>,
    pub location: SourceLocation,
}
