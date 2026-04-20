use serde::{Deserialize, Serialize};

use crate::model::{SourceLocation, TypeInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: Option<String>,
    pub type_info: TypeInfo,
    pub location: SourceLocation,
}
