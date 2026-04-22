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
pub struct SwitchCase {
    /// The case label (case PROP_FOO:, default:, etc.)
    pub label: CaseLabel,
    /// Statements between this case and the next case/end
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchStatement {
    pub condition: Expression,
    pub condition_location: SourceLocation,
    /// Cases with their associated statement bodies
    pub cases: Vec<SwitchCase>,
    pub location: SourceLocation,
}

impl SwitchStatement {
    /// Extract identifiers from non-default case labels
    /// Returns vector of case value identifier names (e.g., ["PROP_NAME",
    /// "PROP_TITLE"])
    pub fn case_identifiers(&self) -> Vec<String> {
        self.cases
            .iter()
            .filter_map(|case| case.label.value.as_ref())
            .filter_map(|expr| {
                if let Expression::Identifier(id) = expr {
                    Some(id.name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Check if switch has a default case
    pub fn has_default_case(&self) -> bool {
        self.default_case().is_some()
    }

    /// Get all statements across all case bodies (flattened view)
    pub fn all_statements(&self) -> impl Iterator<Item = &Statement> {
        self.cases.iter().flat_map(|case| case.body.iter())
    }

    /// Find the default case if it exists
    pub fn default_case(&self) -> Option<&SwitchCase> {
        self.cases.iter().find(|case| case.label.value.is_none())
    }
}
