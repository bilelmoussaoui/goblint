use serde::{Deserialize, Serialize};

use crate::model::{SourceLocation, expression::Expression};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializerListExpression {
    pub items: Vec<InitializerItem>,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializerItem {
    /// Optional designator (.field or [index])
    pub designator: Option<Designator>,
    /// The value expression
    pub value: Box<Expression>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Designator {
    /// Field designator: .field_name
    Field(String),
    /// Array/subscript designator: [index]
    Subscript(Box<Expression>),
}
