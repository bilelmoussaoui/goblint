use serde::{Deserialize, Serialize};

use crate::model::{SourceLocation, expression::Expression, statement::Statement};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForStatement {
    /// Initializer expression (can be assignment or declaration)
    pub initializer: Option<Box<Expression>>,
    /// Condition expression
    pub condition: Option<Box<Expression>>,
    /// Update expression
    pub update: Option<Box<Expression>>,
    /// Loop body (can be single statement or compound)
    pub body: Vec<Statement>,
    pub location: SourceLocation,
}
