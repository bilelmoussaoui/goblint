use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

/// The complete project model - a map of files to their content
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Project {
    pub files: HashMap<PathBuf, FileModel>,
}

impl Project {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    /// Get a file's model
    pub fn get_file(&self, path: &PathBuf) -> Option<&FileModel> {
        self.files.get(path)
    }

    /// Find a function by name across all files
    pub fn find_function(&self, name: &str) -> Option<&FunctionInfo> {
        for file in self.files.values() {
            if let Some(func) = file.functions.iter().find(|f| f.name == name) {
                return Some(func);
            }
        }
        None
    }

    /// Check if a function is declared in any header
    pub fn is_function_declared_in_header(&self, name: &str) -> bool {
        for file in self.files.values() {
            if file.path.extension().map_or(false, |ext| ext == "h") {
                if file.functions.iter().any(|f| f.name == name) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if a function has export macros (truly public API)
    pub fn is_function_exported(&self, name: &str) -> bool {
        for file in self.files.values() {
            if let Some(func) = file.functions.iter().find(|f| f.name == name) {
                if !func.export_macros.is_empty() {
                    return true;
                }
            }
        }
        false
    }
}

/// Model of a single file (header or C file)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileModel {
    pub path: PathBuf,
    pub includes: Vec<Include>,
    pub typedefs: Vec<TypedefInfo>,
    pub structs: Vec<StructInfo>,
    pub enums: Vec<EnumInfo>,
    pub functions: Vec<FunctionInfo>,
    pub gobject_types: Vec<GObjectType>,
    /// The raw source code of this file - available for detailed pattern
    /// matching
    #[serde(skip)]
    pub source: Vec<u8>,
}

impl FileModel {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            includes: Vec::new(),
            typedefs: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            functions: Vec::new(),
            gobject_types: Vec::new(),
            source: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GObjectType {
    pub type_name: String,  // e.g., "ClutterInputDeviceTool"
    pub type_macro: String, // e.g., "CLUTTER_TYPE_INPUT_DEVICE_TOOL"
    pub kind: GObjectTypeKind,
    pub class_struct: Option<ClassStruct>, // For derivable types
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassStruct {
    pub name: String, // e.g., "CoglWinsysClass"
    pub vfuncs: Vec<VirtualFunction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualFunction {
    pub name: String,
    pub return_type: Option<String>,
    pub parameters: Vec<Parameter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GObjectTypeKind {
    DeclareFinal {
        function_prefix: String, // e.g., "clutter_input_device_tool"
        module_prefix: String,   // e.g., "CLUTTER"
        type_prefix: String,     // e.g., "INPUT_DEVICE_TOOL"
        parent_type: String,     // e.g., "GObject"
    },
    DeclareDerivable {
        function_prefix: String,
        module_prefix: String,
        type_prefix: String,
        parent_type: String,
    },
    DeclareInterface {
        function_prefix: String,
        module_prefix: String,
        type_prefix: String,
        prerequisite_type: String,
    },
    DefineType {
        function_prefix: String,
        parent_type: String,
    },
    DefineTypeWithPrivate {
        function_prefix: String,
        parent_type: String,
    },
    DefineAbstractType {
        function_prefix: String,
        parent_type: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Include {
    pub path: String,
    pub is_system: bool, // <> vs ""
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub name: String,
    pub line: usize,
    pub is_static: bool,
    pub export_macros: Vec<String>, // CLUTTER_EXPORT, G_MODULE_EXPORT, G_DEPRECATED_FOR, etc.
    pub has_static_forward_decl: bool, // Has a static forward declaration in the same file
    pub is_definition: bool,        // true = definition, false = declaration
    pub return_type: Option<String>,
    pub parameters: Vec<Parameter>,
    /// Byte range of the entire function (for definitions) - use with
    /// FileModel.source
    pub start_byte: Option<usize>,
    pub end_byte: Option<usize>,
    /// Byte range of just the function body (for definitions) - use with
    /// FileModel.source
    pub body_start_byte: Option<usize>,
    pub body_end_byte: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: Option<String>,
    pub type_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructInfo {
    pub name: String,
    pub line: usize,
    pub fields: Vec<Field>,
    pub is_opaque: bool, // Only declared, not defined
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumInfo {
    pub name: String,
    pub line: usize,
    pub values: Vec<EnumValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumValue {
    pub name: String,
    pub value: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedefInfo {
    pub name: String,
    pub line: usize,
    pub target_type: String,
}
