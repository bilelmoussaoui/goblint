use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use super::{Comment, top_level::TopLevelItem};

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
    /// All comments in the file, in source order
    pub comments: Vec<Comment>,
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
            comments: Vec::new(),
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

    /// Iterate through class_init functions
    pub fn iter_class_init_functions(
        &self,
    ) -> impl Iterator<Item = &super::top_level::FunctionDefItem> + '_ {
        self.iter_function_definitions()
            .filter(|f| f.name.ends_with("_class_init"))
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

    /// Iterate through property enums (enums that appear to define GObject
    /// properties) Filters for enums where first member starts with PROP_
    /// or ends with _PROP_0
    pub fn iter_property_enums(&self) -> impl Iterator<Item = &super::types::EnumInfo> + '_ {
        self.iter_all_enums().filter(|e| e.is_property_enum())
    }

    /// Find array declarations of a specific type, optionally filtered by
    /// sentinel
    ///
    /// If sentinel_name is None, returns ALL arrays of that type.
    ///
    /// Examples:
    /// - `find_typed_arrays("GParamSpec", true, Some("N_PROPS"))` finds
    ///   `GParamSpec *props[N_PROPS]`
    /// - `find_typed_arrays("GParamSpec", true, None)` finds ALL `GParamSpec
    ///   *[]` arrays
    /// - `find_typed_arrays("guint", false, Some("N_SIGNALS"))` finds `guint
    ///   signals[N_SIGNALS]`
    pub fn find_typed_arrays(
        &self,
        base_type: &str,
        is_pointer: bool,
        sentinel_name: Option<&str>,
    ) -> Vec<&super::statement::VariableDecl> {
        let mut arrays = Vec::new();

        for item in &self.top_level_items {
            self.find_typed_arrays_in_item(item, base_type, is_pointer, sentinel_name, &mut arrays);
        }

        arrays
    }

    fn find_typed_arrays_in_item<'a>(
        &self,
        item: &'a TopLevelItem,
        base_type: &str,
        is_pointer: bool,
        sentinel_name: Option<&str>,
        arrays: &mut Vec<&'a super::statement::VariableDecl>,
    ) {
        use super::{
            Statement,
            expression::Expression,
            top_level::{PreprocessorDirective, TopLevelItem},
        };

        match item {
            TopLevelItem::Declaration(Statement::Declaration(decl))
                if decl.type_info.is_base_type(base_type)
                    && decl.type_info.is_pointer() == is_pointer =>
            {
                let matches = match &decl.array_size {
                    Some(Expression::Identifier(size_id)) => {
                        sentinel_name.map_or(true, |s| size_id.name == s)
                    }
                    Some(Expression::Binary(_)) => {
                        // Binary expressions like PROP_X + 1 - match if no specific sentinel
                        // requested
                        sentinel_name.is_none()
                    }
                    Some(_) => sentinel_name.is_none(),
                    None => false,
                };
                if matches {
                    arrays.push(decl);
                }
            }
            TopLevelItem::Preprocessor(PreprocessorDirective::Conditional { body, .. }) => {
                for nested_item in body {
                    self.find_typed_arrays_in_item(
                        nested_item,
                        base_type,
                        is_pointer,
                        sentinel_name,
                        arrays,
                    );
                }
            }
            _ => {}
        }
    }

    /// Find the class_init function that corresponds to a property enum
    /// Returns the function and its param_spec assignments
    pub fn find_class_init_for_property_enum(
        &self,
        enum_info: &super::types::EnumInfo,
    ) -> Option<(
        &super::top_level::FunctionDefItem,
        Vec<super::types::ParamSpecAssignment>,
    )> {
        use super::{expression::Expression, types::ParamSpecAssignment};

        // Get N_PROPS name if present
        let n_props_name = enum_info
            .values
            .last()
            .filter(|v| v.is_prop_last())
            .map(|v| v.name.as_str());

        // Get all property enum value names (excluding PROP_0 and sentinels)
        let property_names: Vec<&str> = enum_info
            .values
            .iter()
            .filter(|v| !v.is_prop_0() && !v.is_prop_last())
            .map(|v| v.name.as_str())
            .collect();

        // Find GParamSpec array declarations that use N_PROPS or properties from this
        // enum
        let arrays = self.find_typed_arrays("GParamSpec", true, n_props_name);

        // Filter arrays to only those that reference properties from THIS enum
        let arrays: Vec<_> = arrays
            .into_iter()
            .filter(|decl| {
                // If we have an N_PROPS sentinel, we already filtered correctly above
                if n_props_name.is_some() {
                    return true;
                }

                // For modern enums (no N_PROPS), check if the array size uses a property from
                // this enum
                if let Some(Expression::Binary(binary)) = &decl.array_size
                    && let Expression::Identifier(prop_id) = &*binary.left
                {
                    property_names.contains(&prop_id.name.as_str())
                } else {
                    false
                }
            })
            .collect();

        let array_names: Vec<&str> = arrays.iter().map(|d| d.name.as_str()).collect();

        // Find class_init function that uses this array OR property names
        for func in self.iter_class_init_functions() {
            let assignments = func.find_param_spec_assignments(&self.source);

            // Check if any param_spec assignments use our array or enum values
            let has_param_spec_usage = assignments.iter().any(|a| match a {
                ParamSpecAssignment::ArraySubscript { array_name, .. } => {
                    // Match ONLY by array name - the array is the definitive link
                    array_names.contains(&array_name.as_str())
                }
                ParamSpecAssignment::OverrideProperty { enum_value, .. } => {
                    // Override properties don't use arrays - match by enum value
                    property_names.contains(&enum_value.as_str())
                }
                ParamSpecAssignment::Variable { install_call, .. } => {
                    // Variable assignments without arrays - match by enum value in install call
                    // Only relevant when no arrays are found for this enum
                    array_names.is_empty()
                        && install_call.as_ref().is_some_and(|call| {
                            call.get_arg(1)
                                .and_then(|arg| arg.to_source_string(&self.source))
                                .is_some_and(|enum_val| property_names.contains(&enum_val.as_str()))
                        })
                }
            });

            // Also check for install_properties calls (even without param_spec assignments)
            let has_install_call = !array_names.is_empty()
                && func.find_install_properties_calls().iter().any(|call| {
                    call.get_arg(2)
                        .and_then(|arg| arg.to_source_string(&self.source))
                        .is_some_and(|name| array_names.contains(&name.as_str()))
                });

            if has_param_spec_usage || has_install_call {
                return Some((func, assignments));
            }
        }

        None
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
