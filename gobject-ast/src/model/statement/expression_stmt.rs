use serde::{Deserialize, Serialize};

use crate::model::{Expression, SourceLocation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionStmt {
    pub expr: Expression,
    pub location: SourceLocation,
}
