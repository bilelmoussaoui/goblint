use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{ast_context::AstContext, config::Config};

/// Rule category (similar to Clippy's lint categories)
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Serialize)]
pub enum Category {
    /// Code that is outright wrong or very useless
    Correctness,
    /// Code that is most likely wrong or useless
    Suspicious,
    /// Code that should be written in a more idiomatic way
    Style,
    /// Code that does something simple but in a complex way
    Complexity,
    /// Code that can be written to run faster
    Perf,
    /// Lints which are rather strict or have occasional false positives
    Pedantic,
    /// Lints which prevent the use of language/library features
    Restriction,
    /// Code that may cause portability issues across platforms/compilers
    Portability,
}

impl Category {
    pub fn as_str(&self) -> &'static str {
        match self {
            Category::Correctness => "correctness",
            Category::Suspicious => "suspicious",
            Category::Style => "style",
            Category::Complexity => "complexity",
            Category::Perf => "perf",
            Category::Pedantic => "pedantic",
            Category::Restriction => "restriction",
            Category::Portability => "portability",
        }
    }
}

/// Represents an automated fix for a violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fix {
    /// Byte offset where the fix starts
    pub start_byte: usize,
    /// Byte offset where the fix ends (exclusive)
    pub end_byte: usize,
    /// Replacement text
    pub replacement: String,
}

impl Fix {
    /// Create a fix from absolute byte offsets
    pub fn new(start_byte: usize, end_byte: usize, replacement: impl Into<String>) -> Self {
        Self {
            start_byte,
            end_byte,
            replacement: replacement.into(),
        }
    }

    /// Create a fix that deletes an entire line (including indentation and
    /// newline)
    pub fn delete_line(location: &gobject_ast::SourceLocation, source: &[u8]) -> Self {
        // Find the start of the line (rewind to previous newline or start of file)
        let mut line_start = location.start_byte;
        while line_start > 0 && source[line_start - 1] != b'\n' {
            line_start -= 1;
        }

        // Find the end of the line (advance to next newline, including it)
        let mut line_end = location.end_byte;
        while line_end < source.len() && source[line_end] != b'\n' {
            line_end += 1;
        }
        // Include the newline itself
        if line_end < source.len() && source[line_end] == b'\n' {
            line_end += 1;
        }

        Self::new(line_start, line_end, String::new())
    }
}

/// Configuration option metadata for a rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigOption {
    /// Option name (e.g., "config_header")
    pub name: &'static str,
    /// Option type (e.g., "string", "array<string>", "boolean")
    pub option_type: &'static str,
    /// Default value as a string representation (e.g., "\"config.h\"", "[]")
    pub default_value: &'static str,
    /// Example value for documentation (e.g., "[\"cairo_*\", \"Pango*\"]")
    pub example_value: &'static str,
    /// Description of what this option does
    pub description: &'static str,
}

pub mod dead_code;
pub mod deprecated_add_private;
pub mod g_error_init;
pub mod g_error_leak;
pub mod g_object_virtual_methods_chain_up;
pub mod g_param_spec_null_nick_blurb;
pub mod g_param_spec_static_strings;
pub mod g_source_id_not_stored;
pub mod g_task_source_tag;
pub mod include_order;
pub mod matching_declare_define;
pub mod missing_autoptr_cleanup;
pub mod missing_export_macro;
pub mod missing_implementation;
pub mod no_g_auto_macros;
pub mod property_canonical_name;
pub mod property_enum_convention;
pub mod property_enum_coverage;
pub mod property_switch_exhaustiveness;
pub mod signal_canonical_name;
pub mod signal_enum_coverage;
pub mod strcmp_explicit_comparison;
pub mod unnecessary_null_check;
pub mod untranslated_string;
pub mod use_clear_functions;
pub mod use_explicit_default_flags;
pub mod use_g_ascii_functions;
pub mod use_g_autofree;
pub mod use_g_autolist;
pub mod use_g_autoptr_error;
pub mod use_g_autoptr_goto_cleanup;
pub mod use_g_autoptr_inline_cleanup;
pub mod use_g_bytes_unref_to_data;
pub mod use_g_clear_handle_id;
pub mod use_g_clear_list;
pub mod use_g_clear_signal_handler;
pub mod use_g_clear_weak_pointer;
pub mod use_g_file_load_bytes;
pub mod use_g_gnuc_flag_enum;
pub mod use_g_new;
pub mod use_g_object_class_install_properties;
pub mod use_g_object_new_with_properties;
pub mod use_g_object_notify_by_pspec;
pub mod use_g_set_object;
pub mod use_g_set_str;
pub mod use_g_settings_typed;
pub mod use_g_source_constants;
pub mod use_g_source_once;
pub mod use_g_steal_pointer;
pub mod use_g_str_has_prefix_suffix;
pub mod use_g_strcmp0;
pub mod use_g_string_free_and_steal;
pub mod use_g_strlcpy;
pub mod use_g_value_set_static_string;
pub mod use_g_variant_new_typed;
pub mod use_pragma_once;

pub use dead_code::DeadCode;
pub use deprecated_add_private::DeprecatedAddPrivate;
pub use g_error_init::GErrorInit;
pub use g_error_leak::GErrorLeak;
pub use g_object_virtual_methods_chain_up::GObjectVirtualMethodsChainUp;
pub use g_param_spec_null_nick_blurb::GParamSpecNullNickBlurb;
pub use g_param_spec_static_strings::GParamSpecStaticStrings;
pub use g_source_id_not_stored::GSourceIdNotStored;
pub use g_task_source_tag::GTaskSourceTag;
pub use include_order::IncludeOrder;
pub use matching_declare_define::MatchingDeclareDefine;
pub use missing_autoptr_cleanup::MissingAutoptrCleanup;
pub use missing_export_macro::MissingExportMacro;
pub use missing_implementation::MissingImplementation;
pub use no_g_auto_macros::NoGAutoMacros;
pub use property_canonical_name::PropertyCanonicalName;
pub use property_enum_convention::PropertyEnumConvention;
pub use property_enum_coverage::PropertyEnumCoverage;
pub use property_switch_exhaustiveness::PropertySwitchExhaustiveness;
pub use signal_canonical_name::SignalCanonicalName;
pub use signal_enum_coverage::SignalEnumCoverage;
pub use strcmp_explicit_comparison::StrcmpExplicitComparison;
pub use unnecessary_null_check::UnnecessaryNullCheck;
pub use untranslated_string::UntranslatedString;
pub use use_clear_functions::UseClearFunctions;
pub use use_explicit_default_flags::UseExplicitDefaultFlags;
pub use use_g_ascii_functions::UseGAsciiFunctions;
pub use use_g_autofree::UseGAutofree;
pub use use_g_autolist::UseGAutolist;
pub use use_g_autoptr_error::UseGAutoptrError;
pub use use_g_autoptr_goto_cleanup::UseGAutoptrGotoCleanup;
pub use use_g_autoptr_inline_cleanup::UseGAutoptrInlineCleanup;
pub use use_g_bytes_unref_to_data::UseGBytesUnrefToData;
pub use use_g_clear_handle_id::UseGClearHandleId;
pub use use_g_clear_list::UseGClearList;
pub use use_g_clear_signal_handler::UseGClearSignalHandler;
pub use use_g_clear_weak_pointer::UseGClearWeakPointer;
pub use use_g_file_load_bytes::UseGFileLoadBytes;
pub use use_g_gnuc_flag_enum::UseGGnucFlagEnum;
pub use use_g_new::UseGNew;
pub use use_g_object_class_install_properties::UseGObjectClassInstallProperties;
pub use use_g_object_new_with_properties::UseGObjectNewWithProperties;
pub use use_g_object_notify_by_pspec::UseGObjectNotifyByPspec;
pub use use_g_set_object::UseGSetObject;
pub use use_g_set_str::UseGSetStr;
pub use use_g_settings_typed::UseGSettingsTyped;
pub use use_g_source_constants::UseGSourceConstants;
pub use use_g_source_once::UseGSourceOnce;
pub use use_g_steal_pointer::UseGStealPointer;
pub use use_g_str_has_prefix_suffix::UseGStrHasPrefixSuffix;
pub use use_g_strcmp0::UseGStrcmp0;
pub use use_g_string_free_and_steal::UseGStringFreeAndSteal;
pub use use_g_strlcpy::UseGStrlcpy;
pub use use_g_value_set_static_string::UseGValueSetStaticString;
pub use use_g_variant_new_typed::UseGVariantNewTyped;
pub use use_pragma_once::UsePragmaOnce;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Violation {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub rule: &'static str,
    pub category: Category,
    pub level: crate::config::RuleLevel,
    pub snippet: Option<String>,
    /// Rule execution order - higher means more specific/later rules take
    /// precedence
    pub rule_index: usize,
    /// Optional automated fixes (multiple edits can be applied)
    pub fixes: Vec<Fix>,
}

/// Trait that all linting rules must implement
pub trait Rule: Send + Sync {
    /// The unique identifier for this rule (e.g., "missing_implementation")
    fn name(&self) -> &'static str;

    /// Human-readable description of what this rule checks
    fn description(&self) -> &'static str;

    /// Long-form markdown documentation (optional)
    fn long_description(&self) -> Option<&'static str> {
        None
    }

    /// Rule category
    fn category(&self) -> Category;

    /// Whether this rule supports automated fixes via --fix
    fn fixable(&self) -> bool {
        false
    }

    /// Whether this rule requires meson introspection to produce results.
    /// Rules returning true silently skip when no build directory is found.
    fn requires_meson(&self) -> bool {
        false
    }

    /// Configuration options supported by this rule
    fn config_options(&self) -> &'static [ConfigOption] {
        &[]
    }

    /// Check a function implementation (from C files)
    /// Override this to check function bodies and implementations
    #[allow(unused_variables)]
    fn check_func_impl(
        &self,
        ast_context: &AstContext,
        config: &Config,
        func: &gobject_ast::top_level::FunctionDefItem,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        // Default: no-op
    }

    /// Check a function declaration (from header files)
    /// Override this to check function declarations and signatures
    #[allow(unused_variables)]
    fn check_func_decl(
        &self,
        ast_context: &AstContext,
        config: &Config,
        func: &gobject_ast::top_level::FunctionDeclItem,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        // Default: no-op
    }

    /// Check the AST and add violations to the provided vector
    /// Default implementation calls check_func_impl for C files and
    /// check_func_decl for headers Override this if you need custom
    /// iteration logic beyond per-function checking
    fn check_all(
        &self,
        ast_context: &AstContext,
        config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        // Check function implementations in C files
        for (path, file) in ast_context.iter_c_files() {
            for func in file.iter_function_definitions() {
                self.check_func_impl(ast_context, config, func, path, violations);
            }
        }

        // Check function declarations in header files
        for (path, file) in ast_context.iter_header_files() {
            for func in file.iter_function_declarations() {
                self.check_func_decl(ast_context, config, func, path, violations);
            }
        }
    }

    /// Helper to create a violation with the rule name automatically filled in
    fn violation(
        &self,
        file: &std::path::Path,
        line: usize,
        column: usize,
        message: String,
    ) -> Violation {
        Violation {
            file: file.to_path_buf(),
            line,
            column,
            message,
            rule: self.name(),
            category: self.category(),
            level: crate::config::RuleLevel::Error, // Will be overridden by scanner
            snippet: None,
            rule_index: 0, // Will be set by scanner based on execution order
            fixes: Vec::new(),
        }
    }

    /// Helper to create a violation with an automated fix
    fn violation_with_fix(
        &self,
        file: &std::path::Path,
        line: usize,
        column: usize,
        message: String,
        fix: Fix,
    ) -> Violation {
        Violation {
            file: file.to_path_buf(),
            line,
            column,
            message,
            rule: self.name(),
            category: self.category(),
            level: crate::config::RuleLevel::Error, // Will be overridden by scanner
            snippet: None,
            rule_index: 0,
            fixes: vec![fix],
        }
    }

    /// Helper to create a violation with multiple automated fixes
    fn violation_with_fixes(
        &self,
        file: &std::path::Path,
        line: usize,
        column: usize,
        message: String,
        fixes: Vec<Fix>,
    ) -> Violation {
        Violation {
            file: file.to_path_buf(),
            line,
            column,
            message,
            rule: self.name(),
            category: self.category(),
            level: crate::config::RuleLevel::Error, // Will be overridden by scanner
            snippet: None,
            rule_index: 0,
            fixes,
        }
    }
}
