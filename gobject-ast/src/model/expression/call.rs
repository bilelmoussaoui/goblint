use serde::{Deserialize, Serialize};

use crate::model::{Expression, SourceLocation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallExpression {
    pub function: String,
    pub arguments: Vec<Argument>,
    pub location: SourceLocation,
}

impl CallExpression {
    /// Get argument as source text
    pub fn get_arg_text(&self, index: usize, source: &[u8]) -> Option<String> {
        self.arguments.get(index)?.to_source_string(source)
    }

    /// Check if this looks like a macro call (ALL_CAPS or ends with _)
    /// Examples: I_, N_, G_STRINGIFY, GINT_TO_POINTER
    pub fn is_likely_macro(&self) -> bool {
        self.function.chars().all(|c| c.is_uppercase() || c == '_') || self.function.ends_with('_')
    }

    /// Extract string literal from argument, unwrapping macro calls like
    /// I_("string") This is useful for g_param_spec calls where the name
    /// might be I_("property-name")
    pub fn extract_string_from_arg(&self, index: usize) -> Option<String> {
        let Argument::Expression(expr) = self.arguments.get(index)?;
        expr.extract_string_value()
    }

    /// Check if this call is a GObject allocation function
    /// Recognizes g_object_new, g_new, and various other allocation patterns
    pub fn is_allocation_call(&self) -> bool {
        matches!(
            self.function.as_str(),
            "g_object_new"
                | "g_object_new_with_properties"
                | "g_type_create_instance"
                | "g_new"
                | "g_new0"
                | "g_try_new"
                | "g_try_new0"
                | "g_malloc"
                | "g_malloc0"
                | "g_strdup"
                | "g_strndup"
                | "g_file_new_for_path"
                | "g_file_new_for_uri"
                | "g_file_new_tmp"
                | "g_variant_new"
                | "g_variant_ref_sink"
                | "g_bytes_new"
                | "g_bytes_new_take"
                | "g_hash_table_new"
                | "g_hash_table_new_full"
                | "g_array_new"
                | "g_ptr_array_new"
                | "g_error_new"
                | "g_error_new_literal"
        ) || self.function.ends_with("_new")
            || self.function.ends_with("_get_instance")
            || self.function.contains("_new_")
            || self.function.contains("_create")
    }

    /// Check if this call is a GObject cleanup/free function
    /// Recognizes g_object_unref, g_free, and various other cleanup patterns
    pub fn is_cleanup_call(&self) -> bool {
        matches!(
            self.function.as_str(),
            "g_object_unref"
                | "g_clear_object"
                | "g_clear_pointer"
                | "g_error_free"
                | "g_clear_error"
                | "g_free"
                | "g_clear_handle_id"
                | "g_clear_signal_handler"
                | "g_list_free"
                | "g_list_free_full"
                | "g_slist_free"
                | "g_slist_free_full"
                | "g_hash_table_unref"
                | "g_hash_table_destroy"
                | "g_bytes_unref"
                | "g_variant_unref"
                | "g_array_unref"
                | "g_array_free"
                | "g_ptr_array_unref"
                | "g_ptr_array_free"
        ) || self.function.ends_with("_unref")
            || self.function.ends_with("_free")
            || self.function.ends_with("_destroy")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Argument {
    Expression(Box<Expression>),
    // Add more specific types as needed
}

impl Argument {
    /// Convert this argument back to source text
    pub fn to_source_string(&self, source: &[u8]) -> Option<String> {
        match self {
            Argument::Expression(expr) => expr.to_source_string(source),
        }
    }

    /// Check if this argument is a string literal or macro wrapping a string
    pub fn is_string_or_macro_string(&self) -> bool {
        let Argument::Expression(expr) = self;
        expr.is_string_or_macro_string()
    }

    /// Check if this argument is NULL
    pub fn is_null(&self) -> bool {
        let Argument::Expression(expr) = self;
        expr.is_null()
    }

    /// Extract string value from this argument, unwrapping macros
    pub fn extract_string_value(&self) -> Option<String> {
        let Argument::Expression(expr) = self;
        expr.extract_string_value()
    }
}
