use serde::{Deserialize, Serialize};

use crate::model::{Expression, SourceLocation, TypeInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDecl {
    pub type_info: TypeInfo,
    pub name: String,
    pub initializer: Option<Expression>,
    /// Array size expression for array declarators (e.g., [N_PROPS])
    pub array_size: Option<Expression>,
    pub location: SourceLocation,
}

impl VariableDecl {
    /// Get the full type name as a string (for backward compatibility)
    pub fn type_name(&self) -> &str {
        &self.type_info.full_text
    }

    /// Check if this is a simple identifier (not a field access like
    /// obj->field)
    pub fn is_simple_identifier(&self) -> bool {
        !self.name.contains("->") && !self.name.contains('.')
    }
}
