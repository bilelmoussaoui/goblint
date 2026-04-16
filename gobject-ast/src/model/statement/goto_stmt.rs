use serde::{Deserialize, Serialize};

use crate::model::SourceLocation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GotoStatement {
    pub label: String,
    pub location: SourceLocation,
}
