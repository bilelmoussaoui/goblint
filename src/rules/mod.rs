pub mod chainup;
pub mod deprecated_add_private;
pub mod g_param_spec;
pub mod gdeclare_semicolon;
pub mod gerror_init;
pub mod gtask_source_tag;
pub mod missing_implementation;
pub mod property_enum_zero;
pub mod unnecessary_null_check;
pub mod use_clear_functions;
pub mod use_g_strcmp0;

#[derive(Debug, Clone)]
pub struct Violation {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub rule: String,
    pub snippet: Option<String>,
}
