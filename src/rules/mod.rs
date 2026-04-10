use std::path::{Path, PathBuf};

use crate::{ast_context::AstContext, config::Config};

/// Rule category (similar to Clippy's lint categories)
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
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
        }
    }
}

/// Represents an automated fix for a violation
#[derive(Debug, Clone)]
pub struct Fix {
    /// Byte offset where the fix starts
    pub start_byte: usize,
    /// Byte offset where the fix ends (exclusive)
    pub end_byte: usize,
    /// Replacement text
    pub replacement: String,
}

/// Context passed to check_node functions to avoid too many arguments
pub struct CheckContext<'a> {
    pub source: &'a [u8],
    pub file_path: &'a Path,
    pub base_line: usize,
    pub base_byte: usize,
}

pub mod deprecated_add_private;
pub mod g_param_spec_null_nick_blurb;
pub mod g_param_spec_static_strings;
pub mod gdeclare_semicolon;
pub mod gerror_init;
pub mod gobject_virtual_methods_chain_up;
pub mod gtask_source_tag;
pub mod matching_declare_define;
pub mod missing_implementation;
pub mod prefer_g_new;
pub mod prefer_g_object_class_install_properties;
pub mod prefer_g_settings_typed;
pub mod prefer_g_value_set_static_string;
pub mod prefer_g_variant_new_typed;
pub mod property_enum_zero;
pub mod strcmp_equal;
pub mod suggest_g_autofree;
pub mod suggest_g_autoptr_goto;
pub mod suggest_g_autoptr_inline;
pub mod suggest_g_source_once;
pub mod unnecessary_null_check;
pub mod use_clear_functions;
pub mod use_explicit_default_flags;
pub mod use_g_clear_error;
pub mod use_g_clear_handle_id;
pub mod use_g_clear_list;
pub mod use_g_clear_weak_pointer;
pub mod use_g_file_load_bytes;
pub mod use_g_object_new_with_properties;
pub mod use_g_object_notify_by_pspec;
pub mod use_g_set_str;
pub mod use_g_source_constants;
pub mod use_g_strcmp0;
pub mod use_g_string_free_and_steal;

pub use deprecated_add_private::DeprecatedAddPrivate;
pub use g_param_spec_null_nick_blurb::GParamSpecNullNickBlurb;
pub use g_param_spec_static_strings::GParamSpecStaticStrings;
pub use gdeclare_semicolon::GDeclareSemicolon;
pub use gerror_init::GErrorInit;
pub use gobject_virtual_methods_chain_up::GObjectVirtualMethodsChainUp;
pub use gtask_source_tag::GTaskSourceTag;
pub use matching_declare_define::MatchingDeclareDefine;
pub use missing_implementation::MissingImplementation;
pub use prefer_g_new::PreferGNew;
pub use prefer_g_object_class_install_properties::PreferGObjectClassInstallProperties;
pub use prefer_g_settings_typed::PreferGSettingsTyped;
pub use prefer_g_value_set_static_string::PreferGValueSetStaticString;
pub use prefer_g_variant_new_typed::PreferGVariantNewTyped;
pub use property_enum_zero::PropertyEnumZero;
pub use strcmp_equal::StrcmpForStringEqual;
pub use suggest_g_autofree::SuggestGAutofree;
pub use suggest_g_autoptr_goto::SuggestGAutoptrGoto;
pub use suggest_g_autoptr_inline::SuggestGAutoptrInline;
pub use suggest_g_source_once::SuggestGSourceOnce;
pub use unnecessary_null_check::UnnecessaryNullCheck;
pub use use_clear_functions::UseClearFunctions;
pub use use_explicit_default_flags::UseExplicitDefaultFlags;
pub use use_g_clear_error::SuggestGAutoptrError;
pub use use_g_clear_handle_id::UseGClearHandleId;
pub use use_g_clear_list::UseGClearList;
pub use use_g_clear_weak_pointer::UseGClearWeakPointer;
pub use use_g_file_load_bytes::UseGFileLoadBytes;
pub use use_g_object_new_with_properties::UseGObjectNewWithProperties;
pub use use_g_object_notify_by_pspec::UseGObjectNotifyByPspec;
pub use use_g_set_str::UseGSetStr;
pub use use_g_source_constants::UseGSourceConstants;
pub use use_g_strcmp0::UseGStrcmp0;
pub use use_g_string_free_and_steal::UseGStringFreeAndSteal;

#[derive(Debug, Clone)]
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
    /// Optional automated fix
    pub fix: Option<Fix>,
}

/// Trait that all linting rules must implement
pub trait Rule {
    /// The unique identifier for this rule (e.g., "missing_implementation")
    fn name(&self) -> &'static str;

    /// Human-readable description of what this rule checks
    fn description(&self) -> &'static str;

    /// Rule category
    fn category(&self) -> Category;

    /// Whether this rule supports automated fixes via --fix
    fn fixable(&self) -> bool {
        false
    }

    /// Check the AST and add violations to the provided vector
    fn check_all(&self, ast_context: &AstContext, config: &Config, violations: &mut Vec<Violation>);

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
            fix: None,
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
            fix: Some(fix),
        }
    }
}
