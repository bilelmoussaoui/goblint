use serde::{Deserialize, Serialize};

use super::{Property, Signal, function::Parameter};
use crate::SourceLocation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GObjectType {
    pub type_name: String,  // e.g., "ClutterInputDeviceTool"
    pub type_macro: String, // e.g., "CLUTTER_TYPE_INPUT_DEVICE_TOOL"
    pub kind: GObjectTypeKind,
    pub class_struct: Option<ClassStruct>, // For derivable types
    pub interfaces: Vec<InterfaceImplementation>, // G_IMPLEMENT_INTERFACE
    pub has_private: bool,                 // G_ADD_PRIVATE in G_DEFINE_TYPE_WITH_CODE
    pub code_block_statements: Vec<super::super::Statement>, // Statements from *_WITH_CODE macros
    pub export_macros: Vec<String>,        // e.g., ["CLUTTER_EXPORT"]
    pub location: SourceLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceImplementation {
    pub interface_type: String, // e.g., "GTK_TYPE_EDITABLE"
    pub init_function: String,  // e.g., "mask_entry_editable_init"
}

impl GObjectType {
    /// Get the function_prefix from this type
    pub fn function_prefix(&self) -> &str {
        match &self.kind {
            GObjectTypeKind::DeclareFinal {
                function_prefix, ..
            }
            | GObjectTypeKind::DeclareDerivable {
                function_prefix, ..
            }
            | GObjectTypeKind::DeclareInterface {
                function_prefix, ..
            }
            | GObjectTypeKind::DefineType {
                function_prefix, ..
            }
            | GObjectTypeKind::DefineTypeWithPrivate {
                function_prefix, ..
            }
            | GObjectTypeKind::DefineAbstractType {
                function_prefix, ..
            }
            | GObjectTypeKind::DefineTypeWithCode {
                function_prefix, ..
            }
            | GObjectTypeKind::DefineFinalType {
                function_prefix, ..
            }
            | GObjectTypeKind::DefineFinalTypeWithCode {
                function_prefix, ..
            }
            | GObjectTypeKind::DefineFinalTypeWithPrivate {
                function_prefix, ..
            }
            | GObjectTypeKind::DefineAbstractTypeWithCode {
                function_prefix, ..
            }
            | GObjectTypeKind::DefineAbstractTypeWithPrivate {
                function_prefix, ..
            }
            | GObjectTypeKind::DefineInterface {
                function_prefix, ..
            }
            | GObjectTypeKind::DefineInterfaceWithCode {
                function_prefix, ..
            }
            | GObjectTypeKind::DefineBoxedType {
                function_prefix, ..
            }
            | GObjectTypeKind::DefineBoxedTypeWithCode {
                function_prefix, ..
            }
            | GObjectTypeKind::DefinePointerType {
                function_prefix, ..
            } => function_prefix,
        }
    }

    /// Get the expected instance init function name based on the
    /// function_prefix
    pub fn init_function_name(&self) -> String {
        format!("{}_init", self.function_prefix())
    }

    /// Get the expected class_init function name based on the function_prefix
    pub fn class_init_function_name(&self) -> String {
        format!("{}_class_init", self.function_prefix())
    }

    /// Get the expected default_init function name for interfaces
    pub fn default_init_function_name(&self) -> String {
        format!("{}_default_init", self.function_prefix())
    }

    /// Check if this is an interface type
    pub fn is_interface(&self) -> bool {
        matches!(
            self.kind,
            GObjectTypeKind::DeclareInterface { .. }
                | GObjectTypeKind::DefineInterface { .. }
                | GObjectTypeKind::DefineInterfaceWithCode { .. }
        )
    }

    /// Extract properties from a class_init function
    /// Looks for *_param_spec_* calls and extracts property metadata
    pub fn extract_properties(
        &self,
        class_init_func: &crate::top_level::FunctionDefItem,
    ) -> Vec<Property> {
        class_init_func
            .find_calls_matching(|name| {
                // Match any function ending with _param_spec_* pattern
                // e.g., g_param_spec_string, cogl_param_spec_color, etc.
                name.contains("_param_spec_")
            })
            .iter()
            .filter_map(|call| Property::from_param_spec_call(call))
            .collect()
    }

    /// Extract signals from a class_init function
    /// Looks for g_signal_new* calls and extracts signal metadata
    pub fn extract_signals(
        &self,
        class_init_func: &crate::top_level::FunctionDefItem,
        source: &[u8],
    ) -> Vec<Signal> {
        class_init_func
            .find_calls_matching(|name| {
                // Match g_signal_new, g_signal_newv, etc.
                name.starts_with("g_signal_new")
            })
            .iter()
            .filter_map(|call| Signal::from_g_signal_new_call(call, source))
            .collect()
    }
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
    DefineTypeWithCode {
        function_prefix: String,
        parent_type: String,
    },
    DefineFinalType {
        function_prefix: String,
        parent_type: String,
    },
    DefineFinalTypeWithCode {
        function_prefix: String,
        parent_type: String,
    },
    DefineFinalTypeWithPrivate {
        function_prefix: String,
        parent_type: String,
    },
    DefineAbstractTypeWithCode {
        function_prefix: String,
        parent_type: String,
    },
    DefineAbstractTypeWithPrivate {
        function_prefix: String,
        parent_type: String,
    },
    DefineInterface {
        function_prefix: String,
        prerequisite_type: String,
    },
    DefineInterfaceWithCode {
        function_prefix: String,
        prerequisite_type: String,
    },
    DefineBoxedType {
        function_prefix: String,
        copy_func: String,
        free_func: String,
    },
    DefineBoxedTypeWithCode {
        function_prefix: String,
        copy_func: String,
        free_func: String,
    },
    DefinePointerType {
        function_prefix: String,
    },
}

impl GObjectTypeKind {
    /// Returns the macro name for this type declaration/definition
    pub fn macro_name(&self) -> &'static str {
        match self {
            Self::DeclareFinal { .. } => "G_DECLARE_FINAL_TYPE",
            Self::DeclareDerivable { .. } => "G_DECLARE_DERIVABLE_TYPE",
            Self::DeclareInterface { .. } => "G_DECLARE_INTERFACE",
            Self::DefineType { .. } => "G_DEFINE_TYPE",
            Self::DefineTypeWithPrivate { .. } => "G_DEFINE_TYPE_WITH_PRIVATE",
            Self::DefineTypeWithCode { .. } => "G_DEFINE_TYPE_WITH_CODE",
            Self::DefineFinalType { .. } => "G_DEFINE_FINAL_TYPE",
            Self::DefineFinalTypeWithCode { .. } => "G_DEFINE_FINAL_TYPE_WITH_CODE",
            Self::DefineFinalTypeWithPrivate { .. } => "G_DEFINE_FINAL_TYPE_WITH_PRIVATE",
            Self::DefineAbstractType { .. } => "G_DEFINE_ABSTRACT_TYPE",
            Self::DefineAbstractTypeWithCode { .. } => "G_DEFINE_ABSTRACT_TYPE_WITH_CODE",
            Self::DefineAbstractTypeWithPrivate { .. } => "G_DEFINE_ABSTRACT_TYPE_WITH_PRIVATE",
            Self::DefineInterface { .. } => "G_DEFINE_INTERFACE",
            Self::DefineInterfaceWithCode { .. } => "G_DEFINE_INTERFACE_WITH_CODE",
            Self::DefineBoxedType { .. } => "G_DEFINE_BOXED_TYPE",
            Self::DefineBoxedTypeWithCode { .. } => "G_DEFINE_BOXED_TYPE_WITH_CODE",
            Self::DefinePointerType { .. } => "G_DEFINE_POINTER_TYPE",
        }
    }

    /// Returns true if this is a G_DECLARE_* macro
    pub fn is_declare(&self) -> bool {
        matches!(
            self,
            Self::DeclareFinal { .. }
                | Self::DeclareDerivable { .. }
                | Self::DeclareInterface { .. }
        )
    }

    /// Returns true if this is a G_DEFINE_* macro
    pub fn is_define(&self) -> bool {
        matches!(
            self,
            Self::DefineType { .. }
                | Self::DefineTypeWithPrivate { .. }
                | Self::DefineTypeWithCode { .. }
                | Self::DefineFinalType { .. }
                | Self::DefineFinalTypeWithCode { .. }
                | Self::DefineFinalTypeWithPrivate { .. }
                | Self::DefineAbstractType { .. }
                | Self::DefineAbstractTypeWithCode { .. }
                | Self::DefineAbstractTypeWithPrivate { .. }
                | Self::DefineInterface { .. }
                | Self::DefineInterfaceWithCode { .. }
                | Self::DefineBoxedType { .. }
                | Self::DefineBoxedTypeWithCode { .. }
        )
    }

    /// Check if a declare kind is compatible with a define kind
    pub fn is_compatible_with(&self, define: &Self) -> bool {
        match self {
            // G_DECLARE_FINAL_TYPE requires a final define so that
            // G_TYPE_FLAG_FINAL is registered at runtime.
            Self::DeclareFinal { .. } => matches!(
                define,
                Self::DefineFinalType { .. }
                    | Self::DefineFinalTypeWithCode { .. }
                    | Self::DefineFinalTypeWithPrivate { .. }
            ),
            // G_DECLARE_DERIVABLE_TYPE covers both concrete and abstract types.
            Self::DeclareDerivable { .. } => matches!(
                define,
                Self::DefineType { .. }
                    | Self::DefineTypeWithCode { .. }
                    | Self::DefineTypeWithPrivate { .. }
                    | Self::DefineAbstractType { .. }
                    | Self::DefineAbstractTypeWithCode { .. }
                    | Self::DefineAbstractTypeWithPrivate { .. }
            ),
            // G_DECLARE_INTERFACE requires G_DEFINE_INTERFACE.
            Self::DeclareInterface { .. } => matches!(
                define,
                Self::DefineInterface { .. } | Self::DefineInterfaceWithCode { .. }
            ),
            _ => false,
        }
    }
}
