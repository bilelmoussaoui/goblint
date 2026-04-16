use serde::{Deserialize, Serialize};

use crate::model::SourceLocation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentifierExpression {
    pub name: String,
    pub location: SourceLocation,
}
