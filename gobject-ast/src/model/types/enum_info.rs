use serde::{Deserialize, Serialize};

use crate::SourceLocation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumInfo {
    pub name: Option<String>,
    pub location: SourceLocation,
    pub values: Vec<EnumValue>,
    /// Location of the enum body for inserting fixes
    pub body_location: SourceLocation,
}

impl EnumInfo {
    pub fn is_property_enum(&self) -> bool {
        self.values
            .iter()
            .any(|v| v.name.contains("_PROP_") || v.name.starts_with("PROP_"))
    }

    pub fn is_signal_enum(&self) -> bool {
        self.values
            .iter()
            .any(|v| v.name.contains("_SIGNAL_") || v.name.starts_with("SIGNAL_"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumValue {
    pub name: String,
    pub value: Option<i64>,
    /// Location of this enumerator node
    pub location: SourceLocation,
    /// Location of just the name
    pub name_location: SourceLocation,
    /// Location of the value (if present)
    pub value_location: Option<SourceLocation>,
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

    /// Check if this is a signal count sentinel (N_SIGNALS, LAST_SIGNAL,
    /// NUM_SIGNALS, etc.)
    pub fn is_signal_last(&self) -> bool {
        self.name.ends_with("_N_SIGNALS")
            || self.name == "N_SIGNALS"
            || self.name.ends_with("_SIGNAL_LAST")
            || self.name == "SIGNAL_LAST"
            || self.name.ends_with("_LAST_SIGNAL")
            || self.name == "LAST_SIGNAL"
            || self.name.ends_with("_NUM_SIGNALS")
            || self.name == "NUM_SIGNALS"
            || (self.name.starts_with("LAST_") && self.name.ends_with("_SIGNAL"))
            || (self.name.starts_with("N_") && self.name.ends_with("_SIGNALS"))
            || (self.name.starts_with("NUM_") && self.name.ends_with("_SIGNALS"))
    }

    /// Extract the value text from source (e.g., for `N_PROPS =
    /// PROP_ORIENTATION`, returns "PROP_ORIENTATION")
    pub fn value_text<'a>(&self, source: &'a [u8]) -> Option<&'a str> {
        self.value_location
            .as_ref()
            .and_then(|loc| loc.as_str(source))
            .map(|s| s.trim())
    }
}
