use serde::{Deserialize, Serialize};

use crate::SourceLocation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumInfo {
    pub name: Option<String>,
    pub location: SourceLocation,
    pub values: Vec<EnumValue>,
    /// Location of the enum body for inserting fixes
    pub body_location: SourceLocation,
    /// Attributes between closing brace and type name (e.g., G_GNUC_FLAG_ENUM)
    pub attributes: Vec<String>,
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

    /// Check if this appears to be a flags enum (bit flags pattern)
    /// based on bit shift operations or power-of-two values
    pub fn is_flags_enum(&self) -> bool {
        if self.values.is_empty() {
            return false;
        }

        let mut has_bit_shift = false;
        let mut has_power_of_two = false;

        for value in &self.values {
            // Check if the value expression is a bit shift operation
            if let Some(expr) = &value.value_expr {
                if is_bit_shift_expr(expr) {
                    has_bit_shift = true;
                }
            }

            // Check if the evaluated value is a power of 2
            if let Some(num) = value.value {
                if num > 0 && (num & (num - 1)) == 0 {
                    has_power_of_two = true;
                }
            }
        }

        // Consider it a flags enum if it uses bit shifts or multiple power-of-two
        // values
        has_bit_shift || (has_power_of_two && self.values.len() > 2)
    }

    /// Check if this enum has a specific attribute (e.g., G_GNUC_FLAG_ENUM)
    pub fn has_attribute(&self, attr_name: &str) -> bool {
        self.attributes.iter().any(|attr| attr == attr_name)
    }
}

/// Check if an expression is a bit shift operation (e.g., 1 << 0)
fn is_bit_shift_expr(expr: &super::super::Expression) -> bool {
    use super::super::operators::BinaryOp;

    match expr {
        super::super::Expression::Binary(bin) => matches!(bin.operator, BinaryOp::LeftShift),
        _ => false,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumValue {
    pub name: String,
    pub value: Option<i64>,
    /// The expression AST for the value (if present)
    pub value_expr: Option<super::super::Expression>,
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
