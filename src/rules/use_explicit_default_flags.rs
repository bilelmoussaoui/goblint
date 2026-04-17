use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseExplicitDefaultFlags;

/// Mapping of (function_name, arg_position, replacement_constant)
/// arg_position is 0-indexed
const FLAG_REPLACEMENTS: &[(&str, usize, &str)] = &[
    // GApplication
    ("g_application_new", 1, "G_APPLICATION_DEFAULT_FLAGS"),
    // GTK bindings
    ("gtk_widget_class_add_binding", 2, "GDK_NO_MODIFIER_MASK"),
    (
        "gtk_widget_class_add_binding_signal",
        2,
        "GDK_NO_MODIFIER_MASK",
    ),
    (
        "gtk_widget_class_add_binding_action",
        2,
        "GDK_NO_MODIFIER_MASK",
    ),
    // GtkShortcut
    ("gtk_shortcut_new", 1, "GDK_NO_MODIFIER_MASK"),
    // GDBus
    ("g_dbus_connection_new", 1, "G_DBUS_CONNECTION_FLAGS_NONE"),
    (
        "g_dbus_connection_new_for_address",
        1,
        "G_DBUS_CONNECTION_FLAGS_NONE",
    ),
    ("g_dbus_proxy_new", 2, "G_DBUS_PROXY_FLAGS_NONE"),
    ("g_dbus_proxy_new_for_bus", 2, "G_DBUS_PROXY_FLAGS_NONE"),
    // GFile
    ("g_file_query_info", 2, "G_FILE_QUERY_INFO_NONE"),
    ("g_file_query_info_async", 2, "G_FILE_QUERY_INFO_NONE"),
    ("g_file_enumerate_children", 1, "G_FILE_QUERY_INFO_NONE"),
    (
        "g_file_enumerate_children_async",
        1,
        "G_FILE_QUERY_INFO_NONE",
    ),
    // GSubprocess
    ("g_subprocess_new", 0, "G_SUBPROCESS_FLAGS_NONE"),
    ("g_subprocess_launcher_new", 0, "G_SUBPROCESS_FLAGS_NONE"),
    // GSettings
    (
        "g_settings_new_with_backend_and_path",
        3,
        "G_SETTINGS_BIND_DEFAULT",
    ),
    // GtkApplication
    ("gtk_application_new", 1, "G_APPLICATION_DEFAULT_FLAGS"),
    // AdwApplication (libadwaita)
    ("adw_application_new", 1, "G_APPLICATION_DEFAULT_FLAGS"),
    // GtkIconTheme (GTK 4.18+)
    ("gtk_icon_theme_lookup_icon", 6, "GTK_ICON_LOOKUP_NONE"),
    ("gtk_icon_theme_lookup_by_gicon", 5, "GTK_ICON_LOOKUP_NONE"),
    // GtkDropTargetAsync (GTK 4.20+)
    ("gtk_drop_target_async_new", 1, "GDK_ACTION_NONE"),
    ("gtk_drop_target_new", 1, "GDK_ACTION_NONE"),
];

impl Rule for UseExplicitDefaultFlags {
    fn name(&self) -> &'static str {
        "use_explicit_default_flags"
    }

    fn description(&self) -> &'static str {
        "Use explicit default flag constants (e.g., G_APPLICATION_DEFAULT_FLAGS) instead of 0"
    }

    fn category(&self) -> super::Category {
        super::Category::Style
    }

    fn fixable(&self) -> bool {
        true
    }

    fn check_func_impl(
        &self,
        _ast_context: &AstContext,
        _config: &Config,
        func: &gobject_ast::top_level::FunctionDefItem,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        // Collect all function names from FLAG_REPLACEMENTS
        let function_names: Vec<&str> = FLAG_REPLACEMENTS.iter().map(|(name, ..)| *name).collect();

        for call in func.find_calls(&function_names) {
            self.check_call(path, call, violations);
        }
    }
}

impl UseExplicitDefaultFlags {
    fn check_call(
        &self,
        file_path: &std::path::Path,
        call: &gobject_ast::CallExpression,
        violations: &mut Vec<Violation>,
    ) {
        // Find the matching replacement rule
        for &(target_func, arg_pos, replacement_const) in FLAG_REPLACEMENTS {
            if call.function == target_func {
                if let Some(arg_expr) = call.get_arg(arg_pos)
                    && arg_expr.is_zero()
                {
                    let fix = Fix::new(
                        arg_expr.location().start_byte,
                        arg_expr.location().end_byte,
                        replacement_const.to_string(),
                    );

                    violations.push(self.violation_with_fix(
                        file_path,
                        call.location.line,
                        call.location.column,
                        format!(
                            "Use {} instead of 0 for {}() flags parameter",
                            replacement_const, target_func
                        ),
                        fix,
                    ));
                }
                break;
            }
        }
    }
}
