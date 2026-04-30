use serde::{Deserialize, Serialize};

use crate::model::{SourceLocation, expression::Expression, statement::Statement};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhileStatement {
    /// Condition expression
    pub condition: Box<Expression>,
    /// Loop body
    pub body: Vec<Statement>,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoWhileStatement {
    /// Loop body
    pub body: Vec<Statement>,
    /// Condition expression
    pub condition: Box<Expression>,
    pub location: SourceLocation,
}
