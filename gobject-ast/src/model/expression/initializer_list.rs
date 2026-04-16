use serde::{Deserialize, Serialize};

use crate::model::SourceLocation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializerListExpression {
    pub text: String, // Full text like "{1, 2, 3}" or "{.x = 1, .y = 2}"
    pub location: SourceLocation,
}
