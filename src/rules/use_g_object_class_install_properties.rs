use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGObjectClassInstallProperties;

impl Rule for UseGObjectClassInstallProperties {
    fn name(&self) -> &'static str {
        "use_g_object_class_install_properties"
    }

    fn description(&self) -> &'static str {
        "Suggest g_object_class_install_properties for multiple g_object_class_install_property calls"
    }

    fn category(&self) -> super::Category {
        super::Category::Pedantic
    }

    fn fixable(&self) -> bool {
        false // Complex refactoring, needs manual intervention
    }

    fn check_func_impl(
        &self,
        _ast_context: &AstContext,
        _config: &Config,
        func: &gobject_ast::top_level::FunctionDefItem,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        // Only check functions ending with _class_init
        if !func.name.ends_with("_class_init") {
            return;
        }

        let install_property_calls = func.find_calls(&["g_object_class_install_property"]);

        if install_property_calls.len() >= 2 {
            let first_call = install_property_calls[0];
            violations.push(self.violation(
                path,
                first_call.location.line,
                first_call.location.column,
                format!(
                    "Consider using g_object_class_install_properties() instead of {} g_object_class_install_property() calls",
                    install_property_calls.len()
                ),
            ));
        }
    }
}
