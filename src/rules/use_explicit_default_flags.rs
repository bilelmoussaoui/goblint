use tree_sitter::Node;

use super::{CheckContext, Fix, Rule};
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

    fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        for (path, file) in ast_context.iter_c_files() {
            for func in &file.functions {
                if !func.is_definition {
                    continue;
                }

                if let Some(func_source) = ast_context.get_function_source(path, func)
                    && let Some(tree) = ast_context.parse_c_source(func_source)
                {
                    let ctx = CheckContext {
                        source: func_source,
                        file_path: path,
                        base_line: func.line,
                        base_byte: func.start_byte.unwrap_or(0),
                    };
                    self.check_node(ast_context, tree.root_node(), &ctx, violations);
                }
            }
        }
    }
}

impl UseExplicitDefaultFlags {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
        {
            let func_name = ast_context.get_node_text(function, ctx.source);

            // Check if this function matches any in our mapping
            for &(target_func, arg_pos, replacement_const) in FLAG_REPLACEMENTS {
                if func_name == target_func {
                    if let Some(args) = node.child_by_field_name("arguments") {
                        let arguments = self.collect_arguments(args);

                        if arg_pos < arguments.len() {
                            let arg_node = arguments[arg_pos];
                            let arg_text = ast_context.get_node_text(arg_node, ctx.source);

                            // Check if the argument is literally "0"
                            if arg_text == "0" {
                                let fix = Fix::from_node(arg_node, ctx, replacement_const);

                                violations.push(self.violation_with_fix(
                                    ctx.file_path,
                                    ctx.base_line + node.start_position().row,
                                    node.start_position().column + 1,
                                    format!(
                                        "Use {} instead of 0 for {}() flags parameter",
                                        replacement_const, target_func
                                    ),
                                    fix,
                                ));
                            }
                        }
                    }
                    break;
                }
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(ast_context, child, ctx, violations);
        }
    }

    fn collect_arguments<'a>(&self, args_node: Node<'a>) -> Vec<Node<'a>> {
        let mut cursor = args_node.walk();
        let mut arguments = Vec::new();
        for child in args_node.children(&mut cursor) {
            if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                arguments.push(child);
            }
        }
        arguments
    }
}
