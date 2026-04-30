use serde::{Deserialize, Serialize};

use crate::{
    model::{SourceLocation, expression::Expression},
    operators::FieldAccessOp,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldAccessExpression {
    pub base: Box<Expression>,
    pub operator: FieldAccessOp,
    pub field: String,
    pub location: SourceLocation,
}

impl FieldAccessExpression {
    /// Get the full text representation
    pub fn text(&self) -> String {
        format!(
            "{}{}{}",
            self.base.to_text(),
            self.operator.as_str(),
            self.field
        )
    }
}
