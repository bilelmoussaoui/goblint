mod chainup;
mod deprecated_add_private;
mod g_param_spec;
mod gerror_init;
mod gtask_source_tag;
mod property_enum_zero;
mod unnecessary_null_check;
mod use_clear_functions;
mod use_g_strcmp0;

use crate::config::Config;
use std::path::Path;
use tree_sitter::Node;

#[derive(Debug, Clone)]
pub struct Violation {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub rule: String,
    pub snippet: Option<String>,
}

pub trait Rule {
    fn name(&self) -> &str;
    fn check(&self, node: Node, source: &[u8], file_path: &Path) -> Vec<Violation>;
    fn is_enabled(&self, config: &Config) -> bool;
}

pub fn get_all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(g_param_spec::GParamSpecNullNickBlurb),
        Box::new(chainup::DisposeFinalizeChainsUp),
        Box::new(use_clear_functions::UseClearFunctions),
        Box::new(use_g_strcmp0::UseGStrcmp0),
        Box::new(property_enum_zero::PropertyEnumZero),
        Box::new(deprecated_add_private::DeprecatedAddPrivate),
        Box::new(gerror_init::GErrorInit),
        Box::new(gtask_source_tag::GTaskSourceTag),
        Box::new(unnecessary_null_check::UnnecessaryNullCheck),
    ]
}
