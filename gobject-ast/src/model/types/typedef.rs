use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedefInfo {
    pub name: String,
    pub line: usize,
    pub target_type: String,
}
