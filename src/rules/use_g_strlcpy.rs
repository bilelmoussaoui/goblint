use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGStrlcpy;

impl Rule for UseGStrlcpy {
    fn name(&self) -> &'static str {
        "use_g_strlcpy"
    }

    fn description(&self) -> &'static str {
        "Use g_strlcpy/g_strlcat instead of unsafe strcpy/strcat/strncat"
    }

    fn category(&self) -> super::Category {
        super::Category::Correctness
    }

    fn fixable(&self) -> bool {
        false
    }

    fn check_func_impl(
        &self,
        _ast_context: &AstContext,
        _config: &Config,
        func: &gobject_ast::top_level::FunctionDefItem,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        for call in func.find_calls(&["strcpy", "strcat", "strncat"]) {
            let message = match call.function.as_str() {
                "strcpy" => {
                    "Use g_strlcpy(dst, src, sizeof(dst)) instead of strcpy — no bounds checking"
                }
                "strcat" => {
                    "Use g_strlcat(dst, src, sizeof(dst)) instead of strcat — no bounds checking"
                }
                "strncat" => {
                    "Use g_strlcat(dst, src, sizeof(dst)) instead of strncat — strncat's n parameter is the max to append, not the buffer size, which is error-prone"
                }
                _ => continue,
            };

            violations.push(self.violation(
                path,
                call.location.line,
                call.location.column,
                message.to_string(),
            ));
        }
    }
}
