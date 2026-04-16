use serde::{Deserialize, Serialize};

use crate::model::SourceLocation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldAccessExpression {
    pub text: String, // Full text like "self->field" or "obj.field"
    pub location: SourceLocation,
}
