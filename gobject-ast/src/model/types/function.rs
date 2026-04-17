use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: Option<String>,
    pub type_name: String,
}
