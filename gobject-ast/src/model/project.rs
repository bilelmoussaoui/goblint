use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use super::types::{EnumInfo, FunctionInfo, GObjectType, StructInfo, TypedefInfo};
use super::types::Include;

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
