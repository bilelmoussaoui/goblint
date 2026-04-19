use serde::{Deserialize, Serialize};

use crate::model::{Expression, SourceLocation, Statement};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseLabel {
    /// The case value expression (e.g., PROP_FOO, 1, N_PROPS + 1)
    /// None for default case
    pub value: Option<Expression>,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchStatement {
    pub condition: Expression,
    pub condition_location: SourceLocation,
    /// Case labels found in this switch
    pub cases: Vec<CaseLabel>,
    pub body: Vec<Statement>,
    pub location: SourceLocation,
}
