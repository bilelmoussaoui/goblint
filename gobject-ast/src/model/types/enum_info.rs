use serde::{Deserialize, Serialize};

use crate::SourceLocation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumInfo {
    pub name: Option<String>,
    pub location: SourceLocation,
    pub values: Vec<EnumValue>,
    /// Byte range of the enum body for inserting fixes
    pub body_start_byte: usize,
    pub body_end_byte: usize,
}

impl EnumInfo {
    pub fn is_property_enum(&self) -> bool {
        self.values
            .iter()
            .any(|v| v.name.contains("_PROP_") || v.name.starts_with("PROP_"))
    }
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

impl EnumValue {
    /// Check if this is a PROP_0 sentinel (PROP_0, *_PROP_0, etc.)
    pub fn is_prop_0(&self) -> bool {
        self.name.ends_with("_PROP_0")
            || self.name == "PROP_0"
            || (self.name.starts_with("PROP_") && self.name.ends_with("_0"))
            || self.name.ends_with("_PROP_ZERO")
            || self.name == "PROP_ZERO"
            || self.name.ends_with("_ROW_PROP_0")
            || self.name == "ROW_PROP_0"
            || self.name.ends_with("_CHILD_PROP_0")
            || self.name == "CHILD_PROP_0"
    }

    /// Check if this is a property count sentinel (N_PROPS, PROP_LAST,
    /// NUM_PROPERTIES, etc.)
    pub fn is_prop_last(&self) -> bool {
        // Sentinels ending with count/last indicators
        self.name.ends_with("_N_PROPS")
            || self.name == "N_PROPS"
            || self.name.ends_with("_PROP_LAST")
            || self.name == "PROP_LAST"
            || self.name.ends_with("_NUM_PROPERTIES")
            || self.name == "NUM_PROPERTIES"
            || self.name == "N_PROPERTIES"
            || self.name.ends_with("_NUM_PROPS")
            // Patterns like LAST_PROP, LAST_FOO_PROP, LAST_PROPERTY, LAST_ROW_PROPERTY
            || (self.name.starts_with("LAST_") && (self.name.ends_with("_PROP") || self.name.ends_with("_PROPERTY")))
            || self.name == "LAST_PROP"
            || self.name == "LAST_PROPERTY"
            // Patterns like N_FOO_PROPS or NUM_FOO_PROPS
            || (self.name.starts_with("N_") && self.name.ends_with("_PROPS"))
            || (self.name.starts_with("N_") && self.name.ends_with("_PROPERTIES"))
            || (self.name.starts_with("NUM_") && self.name.ends_with("_PROPS"))
    }
}
