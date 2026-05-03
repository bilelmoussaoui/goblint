use serde::{Deserialize, Serialize};

use super::SourceLocation;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutoCleanupMacro {
    /// g_autoptr(TypeName)
    Autoptr(String),
    /// g_auto(TypeName)
    Auto(String),
    /// g_autofree
    Autofree,
    /// g_autolist(TypeName)
    Autolist(String),
    /// g_autoslist(TypeName)
    Autoslist(String),
    /// g_autoqueue(TypeName)
    Autoqueue(String),
}

impl AutoCleanupMacro {
    /// Get the macro name as it would appear in documentation
    pub fn name(&self) -> &'static str {
        match self {
            Self::Autoptr(_) => "g_autoptr",
            Self::Auto(_) => "g_auto",
            Self::Autofree => "g_autofree",
            Self::Autolist(_) => "g_autolist",
            Self::Autoslist(_) => "g_autoslist",
            Self::Autoqueue(_) => "g_autoqueue",
        }
    }

    /// Get the type argument for macros that take one (None for g_autofree)
    pub fn type_arg(&self) -> Option<&str> {
        match self {
            Self::Autoptr(t)
            | Self::Auto(t)
            | Self::Autolist(t)
            | Self::Autoslist(t)
            | Self::Autoqueue(t) => Some(t),
            Self::Autofree => None,
        }
    }
}

impl std::fmt::Display for AutoCleanupMacro {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Autoptr(t) => f.write_fmt(format_args!("g_autoptr({t})")),
            Self::Auto(t) => f.write_fmt(format_args!("g_auto({t})")),
            Self::Autofree => f.write_str("g_autofree"),
            Self::Autolist(t) => f.write_fmt(format_args!("g_autolist({t})")),
            Self::Autoslist(t) => f.write_fmt(format_args!("g_autoslist({t})")),
            Self::Autoqueue(t) => f.write_fmt(format_args!("g_autoqueue({t})")),
        }
    }
}

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
    /// Auto-cleanup macro used, if any
    pub auto_cleanup: Option<AutoCleanupMacro>,
}

impl TypeInfo {
    /// Create TypeInfo from a full type string with location information
    /// Automatically filters out storage class specifiers (static, extern,
    /// inline)
    pub fn new(type_string: String, location: SourceLocation) -> Self {
        let trimmed = type_string.trim();

        // Parse auto-cleanup macro first
        let auto_cleanup = Self::parse_auto_cleanup(trimmed);

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
                _ if auto_cleanup.is_some() && part == auto_cleanup.as_ref().unwrap().name() => {
                    // Skip the auto-cleanup macro name (e.g. g_autofree) — it
                    // is already captured in auto_cleanup and must not end up
                    // in base_type (e.g. "g_autofree MyType" → "MyType").
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
            auto_cleanup,
        }
    }

    /// Parse auto-cleanup macro from type string
    fn parse_auto_cleanup(type_str: &str) -> Option<AutoCleanupMacro> {
        // Helper to extract type from macro(Type) pattern
        let extract_type = |prefix: &str| -> Option<String> {
            if let Some(rest) = type_str.strip_prefix(prefix) {
                if let Some(end) = rest.find(')') {
                    let inner = &rest[0..end];
                    return Some(inner.trim().to_string());
                }
            }
            None
        };

        if type_str.contains("g_autofree") {
            Some(AutoCleanupMacro::Autofree)
        } else if let Some(type_arg) = extract_type("g_autoptr(") {
            Some(AutoCleanupMacro::Autoptr(type_arg))
        } else if let Some(type_arg) = extract_type("g_auto(") {
            Some(AutoCleanupMacro::Auto(type_arg))
        } else if let Some(type_arg) = extract_type("g_autolist(") {
            Some(AutoCleanupMacro::Autolist(type_arg))
        } else if let Some(type_arg) = extract_type("g_autoslist(") {
            Some(AutoCleanupMacro::Autoslist(type_arg))
        } else if let Some(type_arg) = extract_type("g_autoqueue(") {
            Some(AutoCleanupMacro::Autoqueue(type_arg))
        } else {
            None
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

    /// Check if the type uses any auto-cleanup macro (g_autoptr, g_autofree,
    /// g_autolist, etc.)
    pub fn uses_auto_cleanup(&self) -> bool {
        self.auto_cleanup.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autofree() {
        let ti = TypeInfo::new("g_autofree char *".to_string(), SourceLocation::default());
        assert_eq!(ti.auto_cleanup, Some(AutoCleanupMacro::Autofree));
        assert_eq!(ti.auto_cleanup.as_ref().unwrap().name(), "g_autofree");
        assert_eq!(ti.auto_cleanup.as_ref().unwrap().type_arg(), None);
    }

    #[test]
    fn test_autoptr() {
        let ti = TypeInfo::new("g_autoptr(GFile) *".to_string(), SourceLocation::default());
        assert_eq!(
            ti.auto_cleanup,
            Some(AutoCleanupMacro::Autoptr("GFile".to_string()))
        );
        assert_eq!(ti.auto_cleanup.as_ref().unwrap().name(), "g_autoptr");
        assert_eq!(ti.auto_cleanup.as_ref().unwrap().type_arg(), Some("GFile"));
    }

    #[test]
    fn test_auto() {
        let ti = TypeInfo::new("g_auto(GString) *".to_string(), SourceLocation::default());
        assert_eq!(
            ti.auto_cleanup,
            Some(AutoCleanupMacro::Auto("GString".to_string()))
        );
        assert_eq!(ti.auto_cleanup.as_ref().unwrap().name(), "g_auto");
        assert_eq!(
            ti.auto_cleanup.as_ref().unwrap().type_arg(),
            Some("GString")
        );
    }

    #[test]
    fn test_autolist() {
        let ti = TypeInfo::new("g_autolist(GFile)".to_string(), SourceLocation::default());
        assert_eq!(
            ti.auto_cleanup,
            Some(AutoCleanupMacro::Autolist("GFile".to_string()))
        );
        assert_eq!(ti.auto_cleanup.as_ref().unwrap().name(), "g_autolist");
        assert_eq!(ti.auto_cleanup.as_ref().unwrap().type_arg(), Some("GFile"));
    }

    #[test]
    fn test_autoslist() {
        let ti = TypeInfo::new("g_autoslist(GFile)".to_string(), SourceLocation::default());
        assert_eq!(
            ti.auto_cleanup,
            Some(AutoCleanupMacro::Autoslist("GFile".to_string()))
        );
        assert_eq!(ti.auto_cleanup.as_ref().unwrap().name(), "g_autoslist");
        assert_eq!(ti.auto_cleanup.as_ref().unwrap().type_arg(), Some("GFile"));
    }

    #[test]
    fn test_autoqueue() {
        let ti = TypeInfo::new("g_autoqueue(GFile)".to_string(), SourceLocation::default());
        assert_eq!(
            ti.auto_cleanup,
            Some(AutoCleanupMacro::Autoqueue("GFile".to_string()))
        );
        assert_eq!(ti.auto_cleanup.as_ref().unwrap().name(), "g_autoqueue");
        assert_eq!(ti.auto_cleanup.as_ref().unwrap().type_arg(), Some("GFile"));
    }

    #[test]
    fn test_no_auto_cleanup() {
        let ti = TypeInfo::new("char *".to_string(), SourceLocation::default());
        assert_eq!(ti.auto_cleanup, None);
        assert!(!ti.uses_auto_cleanup());
    }

    #[test]
    fn test_const_autofree() {
        let ti = TypeInfo::new(
            "const g_autofree char *".to_string(),
            SourceLocation::default(),
        );
        assert_eq!(ti.auto_cleanup, Some(AutoCleanupMacro::Autofree));
        assert!(ti.is_const);
    }
}

#[test]
fn test_autofree_with_type() {
    let ti = TypeInfo::new(
        "g_autofree FuZipFirmwareWriteItem *".to_string(),
        SourceLocation::default(),
    );
    assert_eq!(ti.auto_cleanup, Some(AutoCleanupMacro::Autofree));
    assert_eq!(ti.base_type, "FuZipFirmwareWriteItem");
}
