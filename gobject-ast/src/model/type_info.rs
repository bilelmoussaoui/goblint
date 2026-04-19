use serde::{Deserialize, Serialize};

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
}

impl TypeInfo {
    /// Create TypeInfo from a full type string (for backward compatibility)
    pub fn from_string(type_string: String) -> Self {
        let trimmed = type_string.trim();
        let is_const = trimmed.starts_with("const ");

        // Remove const qualifier
        let without_const = if is_const {
            trimmed.strip_prefix("const ").unwrap_or(trimmed).trim()
        } else {
            trimmed
        };

        // Count pointer depth
        let pointer_depth = without_const.chars().filter(|&c| c == '*').count();

        // Extract base type (remove pointers and trim)
        let base_type = without_const.replace('*', "").trim().to_string();

        Self {
            base_type,
            is_const,
            pointer_depth,
            full_text: type_string,
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
