use serde::{Deserialize, Serialize};

use super::{FunctionInfo, Property, function::Parameter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GObjectType {
    pub type_name: String,  // e.g., "ClutterInputDeviceTool"
    pub type_macro: String, // e.g., "CLUTTER_TYPE_INPUT_DEVICE_TOOL"
    pub kind: GObjectTypeKind,
    pub class_struct: Option<ClassStruct>, // For derivable types
    pub line: usize,
}

impl GObjectType {
    /// Get the expected class_init function name based on the function_prefix
    pub fn class_init_function_name(&self) -> String {
        let function_prefix = match &self.kind {
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
            } => function_prefix,
        };

        format!("{}_class_init", function_prefix)
    }

    /// Extract properties from a class_init function
    /// Looks for *_param_spec_* calls and extracts property metadata
    pub fn extract_properties(&self, class_init_func: &FunctionInfo) -> Vec<Property> {
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
