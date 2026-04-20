use serde::{Deserialize, Serialize};

use super::SourceLocation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeInfo {
    /// The base type without qualifiers or pointers (e.g., "GFile", "int")
    pub base_type: String,
    /// Whether the type has a const qualifier
    pub is_const: bool,
    /// Number of pointer indirections (0 for value, 1 for *, 2 for **)
    pub pointer_depth: usize,
    /// The full type string as it appears in source (e.g., "const GFile *")
    pub full_text: String,
    /// Location of the type in the source code
    pub location: SourceLocation,
}

impl TypeInfo {
    /// Create TypeInfo from a full type string with location information
    /// Automatically filters out storage class specifiers (static, extern,
    /// inline)
    pub fn new(type_string: String, location: SourceLocation) -> Self {
        let trimmed = type_string.trim();

        // Split by whitespace and filter out storage class specifiers
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let mut filtered_parts = Vec::new();
        let mut is_const = false;

        for part in parts {
            match part {
                "static" | "extern" | "inline" => {
                    // Skip storage class specifiers
                }
                "const" => {
                    is_const = true;
                    filtered_parts.push(part);
                }
                _ => {
                    filtered_parts.push(part);
                }
            }
        }

        let cleaned = filtered_parts.join(" ");

        // Remove const qualifier from base type extraction
        let without_const = if is_const {
            cleaned.strip_prefix("const ").unwrap_or(&cleaned).trim()
        } else {
            &cleaned
        };

        // Count pointer depth
        let pointer_depth = without_const.chars().filter(|&c| c == '*').count();

        // Extract base type (remove pointers and trim)
        let base_type = without_const.replace('*', "").trim().to_string();

        Self {
            base_type,
            is_const,
            pointer_depth,
            full_text: cleaned,
            location,
        }
    }

    /// Check if this is a pointer type (at least one level of indirection)
    pub fn is_pointer(&self) -> bool {
        self.pointer_depth > 0
    }

    /// Get the base type without any qualifiers or pointers
    pub fn base_type_name(&self) -> &str {
        &self.base_type
    }

    /// Check if the base type matches the given name
    pub fn is_base_type(&self, name: &str) -> bool {
        self.base_type == name
    }

    /// Check if the type contains the given string (in full text)
    pub fn contains(&self, pattern: &str) -> bool {
        self.full_text.contains(pattern)
    }
}
