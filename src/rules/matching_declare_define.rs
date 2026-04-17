use std::collections::HashMap;

use gobject_ast::GObjectTypeKind;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct MatchingDeclareDefine;

impl Rule for MatchingDeclareDefine {
    fn name(&self) -> &'static str {
        "matching_declare_define"
    }

    fn description(&self) -> &'static str {
        "Ensure G_DECLARE_* and G_DEFINE_* macros are used consistently"
    }

    fn category(&self) -> super::Category {
        super::Category::Pedantic
    }

    fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        // Build a map of type_name -> GObjectTypeKind from all files
        let mut declared_types: HashMap<String, GObjectTypeKind> = HashMap::new();

        // Scan all files for G_DECLARE_* macros (can be in headers or C files)
        for (_path, file) in ast_context.iter_all_files() {
            for gt in file.iter_all_gobject_types() {
                if gt.kind.is_declare() {
                    declared_types.insert(gt.type_name.clone(), gt.kind.clone());
                }
            }
        }

        // Scan C files for mismatched G_DEFINE_* macros
        for (path, file) in ast_context.iter_c_files() {
            for gt in file.iter_all_gobject_types() {
                if gt.kind.is_define() {
                    // Check if there's a matching declaration
                    if let Some(declare_kind) = declared_types.get(&gt.type_name)
                        && !declare_kind.is_compatible_with(&gt.kind)
                    {
                        violations.push(self.violation(
                            path,
                            gt.location.line,
                            1,
                            format!(
                                "'{}' is declared with {} but defined with {}",
                                gt.type_name,
                                declare_kind.macro_name(),
                                gt.kind.macro_name()
                            ),
                        ));
                    }
                }
            }
        }
    }
}
