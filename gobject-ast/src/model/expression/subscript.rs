use serde::{Deserialize, Serialize};

use crate::model::{Expression, SourceLocation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptExpression {
    pub array: Box<Expression>,
    pub index: Box<Expression>,
    pub location: SourceLocation,
}
