use serde::{Deserialize, Serialize};

use crate::model::SourceLocation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinueStatement {
    pub location: SourceLocation,
}
