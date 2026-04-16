use serde::{Deserialize, Serialize};

use crate::model::{Expression, SourceLocation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assignment {
    pub lhs: String,      // Keep simple for now - just variable name
    pub operator: String, // "=", "+=", etc.
    pub rhs: Box<Expression>,
    pub location: SourceLocation,
}
