use serde::{Deserialize, Serialize};

use crate::model::{SourceLocation, Statement};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompoundStatement {
    pub statements: Vec<Statement>,
    pub location: SourceLocation,
}
