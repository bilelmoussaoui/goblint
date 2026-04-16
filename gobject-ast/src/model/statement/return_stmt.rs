use serde::{Deserialize, Serialize};

use crate::model::{Expression, SourceLocation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnStatement {
    pub value: Option<Expression>,
    pub location: SourceLocation,
}
