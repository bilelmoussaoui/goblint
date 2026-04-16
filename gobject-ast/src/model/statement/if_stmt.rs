use serde::{Deserialize, Serialize};

use crate::model::{Expression, SourceLocation, Statement};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IfStatement {
    pub condition: Expression,
    pub then_body: Vec<Statement>,
    pub then_has_braces: bool,
    pub else_body: Option<Vec<Statement>>,
    pub location: SourceLocation,
}
