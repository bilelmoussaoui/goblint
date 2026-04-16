use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Include {
    pub path: String,
    pub is_system: bool, // <> vs ""
    pub line: usize,
}
