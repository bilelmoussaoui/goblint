use serde::{Deserialize, Serialize};

use crate::model::{Expression, SourceLocation, TypeInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastExpression {
    pub type_info: TypeInfo,
    pub operand: Box<Expression>,
    pub location: SourceLocation,
}
