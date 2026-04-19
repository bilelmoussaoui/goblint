use serde::{Deserialize, Serialize};

use crate::model::TypeInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: Option<String>,
    pub type_info: TypeInfo,
}
