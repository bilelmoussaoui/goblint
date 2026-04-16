use serde::{Deserialize, Serialize};

use crate::model::{AssignmentOp, Expression, SourceLocation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assignment {
    pub lhs: String,           // Keep simple for now - just variable name
    pub operator: AssignmentOp,
    pub rhs: Box<Expression>,
    pub location: SourceLocation,
}
