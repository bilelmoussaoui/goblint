use serde::{Deserialize, Serialize};

use crate::model::{Expression, SourceLocation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDecl {
    pub type_name: String,
    pub name: String,
    pub initializer: Option<Expression>,
    pub location: SourceLocation,
}
