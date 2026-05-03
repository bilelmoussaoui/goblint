use serde::{Deserialize, Serialize};

use super::{Property, Signal, function::Parameter};
use crate::SourceLocation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GObjectType {
    pub type_name: String,           // e.g., "ClutterInputDeviceTool"
    pub type_macro: String,          // e.g., "CLUTTER_TYPE_INPUT_DEVICE_TOOL"
    pub function_prefix: String,     // e.g., "clutter_input_device_tool"
    pub parent_type: Option<String>, // e.g., "GObject"; None for boxed/pointer types
    pub flags: Option<String>,       /* G_DEFINE_TYPE_EXTENDED flags arg, e.g.
                                      * "G_TYPE_FLAG_ABSTRACT" */
    pub kind: GObjectTypeKind,
    pub class_struct: Option<ClassStruct>, // For derivable types
    pub interfaces: Vec<InterfaceImplementation>, // G_IMPLEMENT_INTERFACE
    pub has_private: bool,                 /* G_ADD_PRIVATE in *_WITH_CODE, or *_WITH_PRIVATE
                                            * macros */
    pub code_block_statements: Vec<super::super::Statement>, // Statements from *_WITH_CODE macros
    pub export_macros: Vec<String>,                          // e.g., ["CLUTTER_EXPORT"]
    pub location: SourceLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceImplementation {
    pub interface_type: String, // e.g., "GTK_TYPE_EDITABLE"
    pub init_function: String,  // e.g., "mask_entry_editable_init"
}

impl GObjectType {
    pub fn function_prefix(&self) -> &str {
        &self.function_prefix
    }

    /// Get the expected instance init function name based on the
    /// function_prefix
    pub fn init_function_name(&self) -> String {
        format!("{}_init", self.function_prefix)
    }

    /// Get the expected class_init function name based on the function_prefix
    pub fn class_init_function_name(&self) -> String {
        format!("{}_class_init", self.function_prefix)
    }

    /// Get the expected default_init function name for interfaces
    pub fn default_init_function_name(&self) -> String {
        format!("{}_default_init", self.function_prefix)
    }

    /// Check if this is an interface type
    pub fn is_interface(&self) -> bool {
        matches!(
            self.kind,
            GObjectTypeKind::Declare {
                kind: DeclareKind::Interface,
                ..
            } | GObjectTypeKind::Define(DefineKind::Interface | DefineKind::InterfaceWithCode)
        )
    }

    /// Extract properties from a class_init function
    pub fn extract_properties(
        &self,
        class_init_func: &crate::top_level::FunctionDefItem,
    ) -> Vec<Property> {
        class_init_func
            .find_calls_matching(|name| name.contains("_param_spec_"))
            .iter()
            .filter_map(|call| Property::from_param_spec_call(call))
            .collect()
    }

    /// Extract signals from a class_init function
    pub fn extract_signals(
        &self,
        class_init_func: &crate::top_level::FunctionDefItem,
        source: &[u8],
    ) -> Vec<Signal> {
        class_init_func
            .find_calls_matching(|name| name.starts_with("g_signal_new"))
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
    pub return_type: crate::TypeInfo,
    pub parameters: Vec<Parameter>,
}

/// Which G_DECLARE_* variant was used
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeclareKind {
    Final,
    Derivable,
    Interface,
}

/// Which G_DEFINE_* (non-boxed, non-pointer, non-extended) variant was used
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DefineKind {
    Type,
    TypeWithCode,
    TypeWithPrivate,
    FinalType,
    FinalTypeWithCode,
    FinalTypeWithPrivate,
    AbstractType,
    AbstractTypeWithCode,
    AbstractTypeWithPrivate,
    Interface,
    InterfaceWithCode,
    /// G_DEFINE_TYPE_EXTENDED
    TypeExtended,
    /// G_DEFINE_POINTER_TYPE
    Pointer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GObjectTypeKind {
    /// G_DECLARE_FINAL_TYPE / G_DECLARE_DERIVABLE_TYPE / G_DECLARE_INTERFACE
    Declare {
        kind: DeclareKind,
        module_prefix: String, // e.g., "CLUTTER"
        type_prefix: String,   // e.g., "INPUT_DEVICE_TOOL"
    },
    /// All G_DEFINE_TYPE* / G_DEFINE_INTERFACE* variants (not
    /// boxed/pointer/extended)
    Define(DefineKind),
    /// G_DEFINE_BOXED_TYPE / G_DEFINE_BOXED_TYPE_WITH_CODE
    DefineBoxed {
        copy_func: String,
        free_func: String,
    },
}

impl GObjectTypeKind {
    /// Returns the macro name for this type declaration/definition
    pub fn macro_name(&self) -> &'static str {
        match self {
            Self::Declare { kind, .. } => match kind {
                DeclareKind::Final => "G_DECLARE_FINAL_TYPE",
                DeclareKind::Derivable => "G_DECLARE_DERIVABLE_TYPE",
                DeclareKind::Interface => "G_DECLARE_INTERFACE",
            },
            Self::Define(kind) => match kind {
                DefineKind::Type => "G_DEFINE_TYPE",
                DefineKind::TypeWithCode => "G_DEFINE_TYPE_WITH_CODE",
                DefineKind::TypeWithPrivate => "G_DEFINE_TYPE_WITH_PRIVATE",
                DefineKind::FinalType => "G_DEFINE_FINAL_TYPE",
                DefineKind::FinalTypeWithCode => "G_DEFINE_FINAL_TYPE_WITH_CODE",
                DefineKind::FinalTypeWithPrivate => "G_DEFINE_FINAL_TYPE_WITH_PRIVATE",
                DefineKind::AbstractType => "G_DEFINE_ABSTRACT_TYPE",
                DefineKind::AbstractTypeWithCode => "G_DEFINE_ABSTRACT_TYPE_WITH_CODE",
                DefineKind::AbstractTypeWithPrivate => "G_DEFINE_ABSTRACT_TYPE_WITH_PRIVATE",
                DefineKind::Interface => "G_DEFINE_INTERFACE",
                DefineKind::InterfaceWithCode => "G_DEFINE_INTERFACE_WITH_CODE",
                DefineKind::TypeExtended => "G_DEFINE_TYPE_EXTENDED",
                DefineKind::Pointer => "G_DEFINE_POINTER_TYPE",
            },
            Self::DefineBoxed { .. } => "G_DEFINE_BOXED_TYPE",
        }
    }

    /// Returns true if this is a G_DECLARE_* macro
    pub fn is_declare(&self) -> bool {
        matches!(self, Self::Declare { .. })
    }

    /// Returns true if this is a G_DEFINE_* macro
    pub fn is_define(&self) -> bool {
        matches!(self, Self::Define(_) | Self::DefineBoxed { .. })
    }

    /// Check if a declare kind is compatible with a define kind
    pub fn is_compatible_with(&self, define: &Self) -> bool {
        let Self::Declare { kind, .. } = self else {
            return false;
        };
        match kind {
            // G_DECLARE_FINAL_TYPE requires a final define
            DeclareKind::Final => matches!(
                define,
                Self::Define(
                    DefineKind::FinalType
                        | DefineKind::FinalTypeWithCode
                        | DefineKind::FinalTypeWithPrivate
                )
            ),
            // G_DECLARE_DERIVABLE_TYPE covers concrete and abstract types
            DeclareKind::Derivable => matches!(
                define,
                Self::Define(
                    DefineKind::Type
                        | DefineKind::TypeWithCode
                        | DefineKind::TypeWithPrivate
                        | DefineKind::AbstractType
                        | DefineKind::AbstractTypeWithCode
                        | DefineKind::AbstractTypeWithPrivate
                        | DefineKind::TypeExtended
                )
            ),
            // G_DECLARE_INTERFACE requires G_DEFINE_INTERFACE
            DeclareKind::Interface => matches!(
                define,
                Self::Define(DefineKind::Interface | DefineKind::InterfaceWithCode)
            ),
        }
    }
}
