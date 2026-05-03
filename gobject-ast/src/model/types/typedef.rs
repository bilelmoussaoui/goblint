use serde::{Deserialize, Serialize};

use crate::{SourceLocation, TypeInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedefInfo {
    pub name: String,
    pub location: SourceLocation,
    pub target_type: TypeInfo,
    /// Bare tag name for `typedef struct _Foo Foo` / `typedef union _Bar Bar`.
    pub tag_name: Option<String>,
}
