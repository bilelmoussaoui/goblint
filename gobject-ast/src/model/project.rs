use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use super::top_level::TopLevelItem;

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

    /// Check if a function is declared in any header
    pub fn is_function_declared_in_header(&self, name: &str) -> bool {
        for file in self.files.values() {
            if file.path.extension().map_or(false, |ext| ext == "h") {
                if file
                    .iter_function_declarations()
                    .any(|decl| decl.name == name)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Check if a function has export macros (truly public API)
    pub fn is_function_exported(&self, name: &str) -> bool {
        for file in self.files.values() {
            if file
                .iter_function_declarations()
                .any(|decl| decl.name == name && !decl.export_macros.is_empty())
            {
                return true;
            }
        }
        false
    }
}

/// Model of a single file (header or C file)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileModel {
    pub path: PathBuf,
    /// Top-level items in source order (preserves structure like #ifdef blocks)
    pub top_level_items: Vec<TopLevelItem>,
    /// The raw source code of this file - available for detailed pattern
    /// matching
    #[serde(skip)]
    pub source: Vec<u8>,
}

impl FileModel {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            top_level_items: Vec::new(),
            source: Vec::new(),
        }
    }

    /// Iterate through all includes in the file (including those in #ifdef
    /// blocks)
    pub fn iter_all_includes(
        &self,
    ) -> impl Iterator<Item = (&str, bool, crate::SourceLocation)> + '_ {
        self.iter_items_recursive(&self.top_level_items)
            .filter_map(|item| {
                use crate::top_level::PreprocessorDirective;
                match item {
                    TopLevelItem::Preprocessor(PreprocessorDirective::Include {
                        path,
                        is_system,
                        location,
                    }) => Some((path.as_str(), *is_system, *location)),
                    _ => None,
                }
            })
    }

    /// Iterate through all function definitions in the file (including those in
    /// #ifdef blocks)
    pub fn iter_function_definitions(
        &self,
    ) -> impl Iterator<Item = &super::top_level::FunctionDefItem> + '_ {
        self.iter_items_recursive(&self.top_level_items)
            .filter_map(|item| match item {
                TopLevelItem::FunctionDefinition(func) => Some(func),
                _ => None,
            })
    }

    /// Iterate through all function declarations in the file (including those
    /// in #ifdef blocks)
    pub fn iter_function_declarations(
        &self,
    ) -> impl Iterator<Item = &super::top_level::FunctionDeclItem> + '_ {
        self.iter_items_recursive(&self.top_level_items)
            .filter_map(|item| match item {
                TopLevelItem::FunctionDeclaration(func) => Some(func),
                _ => None,
            })
    }

    /// Iterate through all functions (both declarations and definitions),
    /// returning function names
    pub fn iter_all_function_names(&self) -> impl Iterator<Item = &str> + '_ {
        self.iter_items_recursive(&self.top_level_items)
            .filter_map(|item| match item {
                TopLevelItem::FunctionDefinition(func) => Some(func.name.as_str()),
                TopLevelItem::FunctionDeclaration(func) => Some(func.name.as_str()),
                _ => None,
            })
    }

    /// Iterate through all GObject type declarations (including those in #ifdef
    /// blocks)
    pub fn iter_all_gobject_types(&self) -> impl Iterator<Item = &super::types::GObjectType> + '_ {
        self.iter_items_recursive(&self.top_level_items)
            .filter_map(|item| {
                use crate::top_level::PreprocessorDirective;
                match item {
                    TopLevelItem::Preprocessor(PreprocessorDirective::GObjectType {
                        gobject_type,
                        ..
                    }) => Some(gobject_type.as_ref()),
                    _ => None,
                }
            })
    }

    /// Iterate through all enum definitions (including those in #ifdef blocks)
    pub fn iter_all_enums(&self) -> impl Iterator<Item = &super::types::EnumInfo> + '_ {
        self.iter_items_recursive(&self.top_level_items)
            .filter_map(|item| {
                use crate::top_level::TypeDefItem;
                match item {
                    TopLevelItem::TypeDefinition(TypeDefItem::Enum { enum_info }) => {
                        Some(enum_info.as_ref())
                    }
                    _ => None,
                }
            })
    }

    /// Recursively iterate through all items (including those in #ifdef blocks)
    fn iter_items_recursive<'a>(
        &'a self,
        items: &'a [TopLevelItem],
    ) -> Box<dyn Iterator<Item = &'a TopLevelItem> + 'a> {
        use crate::top_level::PreprocessorDirective;

        Box::new(items.iter().flat_map(move |item| match item {
            TopLevelItem::Preprocessor(PreprocessorDirective::Conditional { body, .. }) => {
                Box::new(std::iter::once(item).chain(self.iter_items_recursive(body)))
                    as Box<dyn Iterator<Item = &'a TopLevelItem>>
            }
            _ => Box::new(std::iter::once(item)) as Box<dyn Iterator<Item = &'a TopLevelItem>>,
        }))
    }
}
