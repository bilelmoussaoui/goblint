use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGGnucFlagEnum;

impl Rule for UseGGnucFlagEnum {
    fn name(&self) -> &'static str {
        "use_g_gnuc_flag_enum"
    }

    fn description(&self) -> &'static str {
        "Use G_GNUC_FLAG_ENUM for enums that represent bit flags"
    }

    fn category(&self) -> super::Category {
        super::Category::Style
    }

    fn fixable(&self) -> bool {
        true
    }

    fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        for (path, file) in ast_context.iter_all_files() {
            let source = &ast_context.project.files.get(path).unwrap().source;

            for enum_info in file.iter_all_enums() {
                // Skip anonymous enums (no name)
                let Some(ref enum_name) = enum_info.name else {
                    continue;
                };

                // Check if this enum looks like a flags enum
                if !enum_info.is_flags_enum() {
                    continue;
                }

                // Check if G_GNUC_FLAG_ENUM is already present
                if enum_info.has_attribute("G_GNUC_FLAG_ENUM") {
                    continue;
                }

                // Generate fix: insert G_GNUC_FLAG_ENUM before the type name
                let fix = self.generate_fix(enum_info, source, enum_name);

                violations.push(self.violation_with_fix(
                    path,
                    enum_info.location.line,
                    enum_info.location.column,
                    format!(
                        "Enum '{}' appears to be a flags enum but is missing G_GNUC_FLAG_ENUM attribute",
                        enum_name
                    ),
                    fix,
                ));
            }
        }
    }
}

impl UseGGnucFlagEnum {
    /// Generate a fix to insert G_GNUC_FLAG_ENUM
    fn generate_fix(
        &self,
        enum_info: &gobject_ast::types::EnumInfo,
        source: &[u8],
        enum_name: &str,
    ) -> Fix {
        // We need to find where the type name appears after the closing brace
        // For `typedef enum { ... } Name;` we want to insert before Name
        // For `typedef enum { ... } G_GNUC_FLAG_ENUM Name;` (if already present, but we
        // shouldn't get here)

        let typedef_text = enum_info.location.as_str(source).unwrap_or("");

        // Find the enum name at the end (after the closing brace)
        // Look for `} <possible_spaces> Name;`
        if let Some(closing_brace_pos) = typedef_text.rfind('}') {
            let after_brace = &typedef_text[closing_brace_pos + 1..];

            // Find the position of the enum name
            if let Some(name_offset) = after_brace.find(enum_name) {
                // Calculate absolute position in source
                let insert_pos =
                    enum_info.location.start_byte + closing_brace_pos + 1 + name_offset;

                return Fix::new(insert_pos, insert_pos, "G_GNUC_FLAG_ENUM ".to_string());
            }
        }

        // Fallback: insert at the end of the enum body, before the semicolon
        // This shouldn't normally happen, but just in case
        Fix::new(
            enum_info.location.end_byte - 1,
            enum_info.location.end_byte - 1,
            " G_GNUC_FLAG_ENUM".to_string(),
        )
    }
}
