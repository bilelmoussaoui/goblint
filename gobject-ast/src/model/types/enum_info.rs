use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumInfo {
    pub name: String,
    pub line: usize,
    pub values: Vec<EnumValue>,
    /// Byte range of the enum body for inserting fixes
    pub body_start_byte: usize,
    pub body_end_byte: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumValue {
    pub name: String,
    pub value: Option<i64>,
    /// Byte range of this enumerator node
    pub start_byte: usize,
    pub end_byte: usize,
    /// Byte range of just the name
    pub name_start_byte: usize,
    pub name_end_byte: usize,
    /// Byte range of the value (if present)
    pub value_start_byte: Option<usize>,
    pub value_end_byte: Option<usize>,
}
