use gobject_ast::{Argument, Expression, Statement};

use super::Rule;
use crate::{
    ast_context::AstContext,
    config::Config,
    rules::{Category, Violation},
};

pub struct UntranslatedString;

impl Rule for UntranslatedString {
    fn name(&self) -> &'static str {
        "untranslated_string"
    }

    fn description(&self) -> &'static str {
        "Detect user-visible strings that should be wrapped with gettext"
    }

    fn category(&self) -> Category {
        Category::Pedantic
    }

    fn check_func_impl(
        &self,
        _ast_context: &AstContext,
        _config: &Config,
        func: &gobject_ast::top_level::FunctionDefItem,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        self.check_statements(&func.body_statements, path, violations);
    }
}

impl UntranslatedString {
    fn check_statements(
        &self,
        statements: &[Statement],
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        for stmt in statements {
            // Walk all nested statements (e.g., if/while bodies)
            stmt.walk(&mut |s| {
                // For each statement, walk all its expressions
                s.walk_expressions(&mut |expr| {
                    // Recursively check this expression and all nested expressions
                    expr.walk(&mut |e| {
                        self.check_expression(e, file_path, violations);
                    });
                });
            });
        }
    }

    fn check_expression(
        &self,
        expr: &Expression,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        // Look for function calls
        if let Expression::Call(call) = expr {
            let func_name = call.function.as_str();

            // Check if this is a GTK/Adwaita function that takes user-visible text
            if let Some(arg_index) = self.get_translatable_param(func_name)
                && let Some(arg) = call.arguments.get(arg_index)
            {
                self.check_argument(arg, func_name, file_path, violations);
            }
        }
    }

    fn check_argument(
        &self,
        arg: &Argument,
        func_name: &str,
        file_path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        // Extract the expression from the argument
        let Argument::Expression(arg_expr) = arg;

        // Check if the argument is already a call to gettext function
        if let Expression::Call(call) = &**arg_expr {
            let name = call.function.as_str();
            // Already wrapped in gettext
            if matches!(name, "_" | "gettext" | "N_" | "g_dgettext" | "g_dpgettext2") {
                return;
            }
        }

        // Check if it's a raw string literal
        if let Expression::StringLiteral(string_lit) = &**arg_expr {
            // Skip empty strings - they don't need translation
            if string_lit.value.is_empty() {
                return;
            }

            let location = arg_expr.location();
            violations.push(self.violation(
                file_path,
                location.line,
                location.column,
                format!(
                    "User-visible string in {}() should be wrapped with _(\"...\")",
                    func_name
                ),
            ));
        }
    }

    /// Returns argument index for functions that take translatable strings
    fn get_translatable_param(&self, func_name: &str) -> Option<usize> {
        match func_name {
            // GtkLabel
            "gtk_label_new" => Some(0),
            "gtk_label_set_text" | "gtk_label_set_markup" | "gtk_label_set_label" => Some(1),

            // GtkButton
            "gtk_button_new_with_label" | "gtk_button_new_with_mnemonic" => Some(0),
            "gtk_button_set_label" => Some(1),

            // GtkWindow
            "gtk_window_set_title" => Some(1),

            // GtkHeaderBar
            "gtk_header_bar_set_title" | "gtk_header_bar_set_subtitle" => Some(1),

            // GtkCheckButton
            "gtk_check_button_new_with_label" | "gtk_check_button_new_with_mnemonic" => Some(0),

            // GtkRadioButton
            "gtk_radio_button_new_with_label" | "gtk_radio_button_new_with_mnemonic" => Some(1),

            // GtkEntry
            "gtk_entry_set_placeholder_text" | "gtk_entry_set_text" => Some(1),

            // GtkDialog
            "gtk_dialog_add_button" => Some(1),

            // GtkMessageDialog
            "gtk_message_dialog_new" => Some(4),
            "gtk_message_dialog_set_markup" => Some(1),

            // AdwMessageDialog
            "adw_message_dialog_new"
            | "adw_message_dialog_set_heading"
            | "adw_message_dialog_set_body"
            | "adw_message_dialog_set_body_use_markup" => Some(1),
            "adw_message_dialog_add_response" => Some(2),

            // AdwStatusPage
            "adw_status_page_set_title" | "adw_status_page_set_description" => Some(1),

            // AdwToast
            "adw_toast_new" => Some(0),
            "adw_toast_set_title" | "adw_toast_set_button_label" => Some(1),

            // AdwPreferencesGroup
            "adw_preferences_group_set_title" | "adw_preferences_group_set_description" => Some(1),

            // AdwPreferencesRow
            "adw_preferences_row_set_title" => Some(1),

            // AdwActionRow
            "adw_action_row_set_title" | "adw_action_row_set_subtitle" => Some(1),

            // AdwEntryRow
            "adw_entry_row_set_title" => Some(1),

            // AdwComboRow
            "adw_combo_row_set_title" => Some(1),

            // AdwExpanderRow
            "adw_expander_row_set_title" | "adw_expander_row_set_subtitle" => Some(1),

            // AdwWindowTitle
            "adw_window_title_new" => Some(0),
            "adw_window_title_set_title" | "adw_window_title_set_subtitle" => Some(1),

            _ => None,
        }
    }
}
