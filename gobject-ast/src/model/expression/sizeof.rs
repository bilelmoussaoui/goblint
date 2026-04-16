use serde::{Deserialize, Serialize};

use crate::model::SourceLocation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizeofExpression {
    pub text: String, // Full text like "sizeof(int)" or "sizeof x"
    pub location: SourceLocation,
}
