use serde::{Deserialize, Serialize};

use crate::model::{Expression, SourceLocation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionalExpression {
    pub condition: Box<Expression>,
    pub then_expr: Box<Expression>,
    pub else_expr: Box<Expression>,
    pub location: SourceLocation,
}
