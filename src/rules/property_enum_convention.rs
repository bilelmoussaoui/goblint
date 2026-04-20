use gobject_ast::Expression;

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct PropertyEnumConvention;

/// Context about which class owns a property enum
#[derive(Debug)]
struct ClassContext {
    class_type_info: gobject_ast::TypeInfo,
    get_property_func: Option<String>,
    set_property_func: Option<String>,
}

impl Rule for PropertyEnumConvention {
    fn name(&self) -> &'static str {
        "property_enum_convention"
    }

    fn description(&self) -> &'static str {
        "Prefer modern property enum pattern (PROP_FOO = 1) over legacy PROP_0/N_PROPS pattern"
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
            // Collect sentinel names from enums that will be transformed
            // (skip override pattern enums and already-modern enums)
            let sentinel_usage: std::collections::HashMap<String, usize> = file
                .iter_all_enums()
                .filter(|e| e.is_property_enum())
                .filter(|e| {
                    // Apply same checks as main loop to see if this enum will be transformed
                    let has_prop_0 = e.values.first().map(|v| v.is_prop_0()).unwrap_or(false);

                    let has_n_props_at_end =
                        e.values.last().map(|v| v.is_prop_last()).unwrap_or(false);

                    let has_n_props_in_middle = e
                        .values
                        .iter()
                        .enumerate()
                        .any(|(idx, v)| idx < e.values.len() - 1 && v.is_prop_last());

                    // Only count if it will be transformed (not in-middle pattern, not already
                    // modern) Note: N_PROPS = PROP_X where PROP_X is override
                    // is still transformable
                    !has_n_props_in_middle && (has_prop_0 || has_n_props_at_end)
                })
                .filter_map(|e| {
                    e.values
                        .iter()
                        .find(|v| v.is_prop_last())
                        .map(|v| v.name.clone())
                })
                .fold(std::collections::HashMap::new(), |mut map, name| {
                    *map.entry(name).or_insert(0) += 1;
                    map
                });

            for enum_info in file.iter_all_enums() {
                if !enum_info.is_property_enum() {
                    continue;
                }

                // Check if this uses the old pattern: PROP_0 at start and N_PROPS at end
                let has_prop_0 = enum_info
                    .values
                    .first()
                    .map(|v| v.is_prop_0())
                    .unwrap_or(false);

                let has_n_props = enum_info
                    .values
                    .last()
                    .map(|v| v.is_prop_last())
                    .unwrap_or(false);

                // Check if N_PROPS appears in the middle (not last) - this is the override
                // properties pattern
                let has_n_props_in_middle = enum_info
                    .values
                    .iter()
                    .enumerate()
                    .any(|(idx, v)| idx < enum_info.values.len() - 1 && v.is_prop_last());

                if !has_prop_0 && !has_n_props {
                    // Already using new pattern, skip
                    continue;
                }

                // Get the names we need to work with
                let prop_0_name = enum_info.values.first().unwrap().name.clone();
                let n_props_name = enum_info.values.last().unwrap().name.clone();

                // Skip if this is the interface override pattern:
                // - N_PROPS in the middle
                // - N_PROPS used in switch case expressions
                if has_n_props_in_middle
                    || (has_n_props && self.n_props_used_in_switch_cases(file, &n_props_name))
                {
                    // Skip: interface override pattern detected
                    continue;
                }

                // Build map of which properties are overrides
                let property_map = self.build_property_override_map(file);

                // Get the name of the last REAL property (the one before N_PROPS)
                // If N_PROPS = PROP_X and PROP_X is an override, find the last non-override
                // property
                let last_real_prop_name = if has_n_props && enum_info.values.len() >= 2 {
                    // Check if the value before N_PROPS is an override
                    let second_to_last = &enum_info.values[enum_info.values.len() - 2];

                    // Also check if N_PROPS = PROP_X where PROP_X is an override
                    let n_props_value = enum_info.values.last().unwrap();
                    let n_props_points_to_override = if n_props_value.value_start_byte.is_some()
                        && n_props_value.value.is_none()
                    {
                        // Extract the identifier from source (e.g., "PROP_ORIENTATION")
                        if let (Some(start), Some(end)) =
                            (n_props_value.value_start_byte, n_props_value.value_end_byte)
                        {
                            std::str::from_utf8(&file.source[start..end])
                                .ok()
                                .and_then(|value_text| {
                                    let trimmed = value_text.trim();
                                    property_map.get(trimmed).copied()
                                })
                                .unwrap_or(false)
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if n_props_points_to_override {
                        // N_PROPS = PROP_ORIENTATION (override), so find last non-override property
                        enum_info
                            .values
                            .iter()
                            .rev()
                            .skip(1) // Skip N_PROPS
                            .find(|v| {
                                !v.is_prop_0()
                                    && !v.is_prop_last()
                                    && !property_map.get(&v.name).copied().unwrap_or(false)
                            })
                            .map(|v| v.name.clone())
                            .unwrap_or_else(|| second_to_last.name.clone())
                    } else {
                        second_to_last.name.clone()
                    }
                } else {
                    enum_info.values.last().unwrap().name.clone()
                };

                // Find which class_init and property functions correspond to this enum
                // by tracing N_PROPS through GParamSpec array to class_init
                let class_context = self.find_class_context_for_enum(file, enum_info);

                // IMPORTANT: Only transform if we can verify this is a GObject property enum
                // by finding its class_init. This prevents transforming unrelated enums that
                // happen to have N_PROPS (e.g., in header files, or non-GObject enums).
                if class_context.is_none() {
                    continue;
                }

                // Determine the enum name to use (either from typedef or derived from
                // class_init)
                let derived_enum_name = if enum_info.name.is_none() {
                    class_context
                        .as_ref()
                        .and_then(|ctx| self.derive_enum_name_from_class_type(&ctx.class_type_info))
                } else {
                    None
                };

                let mut fixes = Vec::new();

                // Fix 0: Convert anonymous enum to typedef if needed
                if let Some(ref enum_name) = derived_enum_name {
                    // Add "typedef " before "enum"
                    fixes.push(Fix::new(
                        enum_info.location.start_byte,
                        enum_info.location.start_byte,
                        "typedef ".to_string(),
                    ));

                    // Add enum name and semicolon after the closing brace
                    // Find the semicolon after the enum body
                    let mut semicolon_pos = enum_info.body_end_byte;
                    while semicolon_pos < file.source.len() && file.source[semicolon_pos] != b';' {
                        semicolon_pos += 1;
                    }

                    if semicolon_pos < file.source.len() {
                        // Replace the semicolon with " EnumName;"
                        fixes.push(Fix::new(
                            semicolon_pos,
                            semicolon_pos + 1,
                            format!(" {};", enum_name),
                        ));
                    }
                }

                // Fix 1: Remove PROP_0 line entirely (including any blank line after it)
                if has_prop_0 && enum_info.values.len() >= 2 {
                    let prop_0 = &enum_info.values[0];

                    // Use SourceLocation's find_line_bounds_with_following_blank
                    let location =
                        gobject_ast::SourceLocation::new(0, 0, prop_0.start_byte, prop_0.end_byte);
                    let (line_start, line_end) =
                        location.find_line_bounds_with_following_blank(&file.source);
                    fixes.push(Fix::new(line_start, line_end, String::new()));
                }

                // Fix 2: Add " = 1" to the first real property (second value)
                if has_prop_0 && enum_info.values.len() >= 2 {
                    let first_real = &enum_info.values[1];

                    // If the property already has a value (e.g., "= 0"), remove it first
                    if first_real.value == Some(0)
                        && let Some(value_end) = first_real.value_end_byte
                    {
                        // Remove existing " = 0" or "= 0" and replace with " = 1"
                        fixes.push(Fix::new(
                            first_real.name_end_byte,
                            value_end,
                            " = 1".to_string(),
                        ));
                    } else {
                        // Just insert " = 1" right after the property name
                        fixes.push(Fix::new(
                            first_real.name_end_byte,
                            first_real.name_end_byte,
                            " = 1".to_string(),
                        ));
                    }
                }

                // Fix 3: Remove N_PROPS line entirely (including any blank line before it)
                if has_n_props && enum_info.values.len() >= 2 {
                    let n_props = enum_info.values.last().unwrap();

                    // Use SourceLocation's find_line_bounds which already handles preceding blank
                    // lines
                    let location = gobject_ast::SourceLocation::new(
                        0,
                        0,
                        n_props.start_byte,
                        n_props.end_byte,
                    );
                    let (line_start, line_end) = location.find_line_bounds(&file.source);
                    fixes.push(Fix::new(line_start, line_end, String::new()));
                }

                // Fix 4 & 5: Find GParamSpec arrays and fix both their declarations and
                // install_properties calls
                // Only fix if this sentinel name is unique in the file (avoid ambiguity)
                if has_n_props && sentinel_usage.get(&n_props_name).copied().unwrap_or(0) == 1 {
                    let array_names = self.find_and_fix_param_spec_arrays(
                        file,
                        &n_props_name,
                        &last_real_prop_name,
                        &mut fixes,
                    );

                    // Fix install_properties calls that use these arrays
                    for func in file.iter_function_definitions() {
                        for call in func.find_calls(&["g_object_class_install_properties"]) {
                            // Second argument (index 1) should be N_PROPS
                            if let Some(arg) = call.get_arg(1)
                                && let Some(arg_str) = arg.to_simple_string()
                                && arg_str == n_props_name
                            {
                                // Get the array name from third argument
                                if let Some(array_arg) = call.get_arg(2)
                                    && let Some(array_name) = array_arg.to_simple_string()
                                    && array_names.contains(&array_name.as_str())
                                {
                                    let replacement = format!("G_N_ELEMENTS ({})", array_name);
                                    fixes.push(Fix::new(
                                        arg.location().start_byte,
                                        arg.location().end_byte,
                                        replacement,
                                    ));
                                }
                            }
                        }
                    }
                }

                // Fix 6: Add enum cast to switch statements in get_property/set_property
                // This enables -Wswitch-enum to catch missing properties
                // Only apply to the specific property functions for this enum (from
                // class_context)
                if let Some(ref ctx) = class_context {
                    let enum_name = if let Some(ref name) = enum_info.name {
                        name.clone()
                    } else if let Some(ref derived) = derived_enum_name {
                        derived.clone()
                    } else {
                        // Try to derive from class type if we have it
                        self.derive_enum_name_from_class_type(&ctx.class_type_info)
                            .unwrap_or_else(|| "UnknownProps".to_string())
                    };

                    if !enum_name.is_empty() {
                        if let Some(ref func_name) = ctx.get_property_func {
                            self.add_switch_cast_for_function(
                                file, func_name, &enum_name, &mut fixes,
                            );
                        }
                        if let Some(ref func_name) = ctx.set_property_func {
                            self.add_switch_cast_for_function(
                                file, func_name, &enum_name, &mut fixes,
                            );
                        }
                    }
                }

                if !fixes.is_empty() {
                    let message = if has_prop_0 && has_n_props {
                        format!(
                            "Use modern property enum pattern (remove {}, {}, start from = 1)",
                            prop_0_name, n_props_name
                        )
                    } else if has_prop_0 {
                        format!("Remove {} and start enum from = 1", prop_0_name)
                    } else {
                        format!("Remove {} sentinel", n_props_name)
                    };

                    violations.push(self.violation_with_fixes(
                        path,
                        enum_info.location.line,
                        1,
                        message,
                        fixes,
                    ));
                }
            }
        }
    }
}

impl PropertyEnumConvention {
    /// Find GParamSpec arrays that use N_PROPS, fix their declarations, and
    /// return their names e.g., static GParamSpec *props[N_PROPS] -> static
    /// GParamSpec *props[LAST_PROP + 1]
    fn find_and_fix_param_spec_arrays<'a>(
        &self,
        file: &'a gobject_ast::FileModel,
        n_props_name: &str,
        last_prop_name: &str,
        fixes: &mut Vec<Fix>,
    ) -> Vec<&'a str> {
        let mut array_names = Vec::new();

        // Walk through all top-level items looking for static GParamSpec declarations
        for item in &file.top_level_items {
            self.find_param_spec_arrays_in_item(
                item,
                n_props_name,
                last_prop_name,
                &mut array_names,
                fixes,
            );
        }

        array_names
    }

    fn find_param_spec_arrays_in_item<'a>(
        &self,
        item: &'a gobject_ast::top_level::TopLevelItem,
        n_props_name: &str,
        last_prop_name: &str,
        array_names: &mut Vec<&'a str>,
        fixes: &mut Vec<Fix>,
    ) {
        use gobject_ast::{
            Statement,
            top_level::{PreprocessorDirective, TopLevelItem},
        };

        match item {
            TopLevelItem::Declaration(Statement::Declaration(decl))
                // Check if this is a GParamSpec pointer array
                if decl.type_info.is_base_type("GParamSpec") && decl.type_info.is_pointer()
                => {
                    // Check if it's an array declaration using N_PROPS
                    if let Some(Expression::Identifier(size_id)) = &decl.array_size
                        && size_id.name == n_props_name
                    {
                        // Found it! This is a GParamSpec array using N_PROPS
                        // Fix: Replace N_PROPS with LAST_PROP + 1
                        let replacement = format!("{} + 1", last_prop_name);
                        fixes.push(Fix::new(
                            size_id.location.start_byte,
                            size_id.location.end_byte,
                            replacement,
                        ));

                        // Remember this array name
                        array_names.push(&decl.name);
                    }
                }
            TopLevelItem::Preprocessor(PreprocessorDirective::Conditional { body, .. }) => {
                // Recursively search in conditional blocks
                for nested_item in body {
                    self.find_param_spec_arrays_in_item(
                        nested_item,
                        n_props_name,
                        last_prop_name,
                        array_names,
                        fixes,
                    );
                }
            }
            _ => {}
        }
    }

    /// Add switch cast fix for a specific function
    fn add_switch_cast_for_function(
        &self,
        file: &gobject_ast::FileModel,
        func_name: &str,
        enum_name: &str,
        fixes: &mut Vec<Fix>,
    ) {
        // Find the function definition
        for func in file.iter_function_definitions() {
            if func.name != func_name {
                continue;
            }

            // Find switch statements using iterator to handle nested cases
            for stmt in &func.body_statements {
                for switch_stmt in stmt.iter_switches() {
                    self.add_switch_cast_if_needed(switch_stmt, enum_name, fixes);
                }
            }
        }
    }

    /// Helper to add switch cast if not already present
    fn add_switch_cast_if_needed(
        &self,
        switch_stmt: &gobject_ast::SwitchStatement,
        enum_name: &str,
        fixes: &mut Vec<Fix>,
    ) {
        use gobject_ast::Expression;

        // Check if the condition is already a cast to this enum type
        let already_cast = match &switch_stmt.condition {
            Expression::Cast(cast) => {
                // Check if cast type contains the enum name
                cast.type_info.contains(enum_name)
            }
            _ => false,
        };

        if !already_cast {
            // Add fix to wrap the condition in a cast
            let cast_expr = format!("({}) ", enum_name);
            fixes.push(Fix::new(
                switch_stmt.condition_location.start_byte,
                switch_stmt.condition_location.start_byte,
                cast_expr,
            ));
        }
    }

    /// Derive enum name from class type base
    /// e.g., "ClutterActorClass" -> Some("ClutterActorProps")
    /// e.g., "MyObjectClass" -> Some("MyObjectProps")
    fn derive_enum_name_from_class_type(
        &self,
        type_info: &gobject_ast::TypeInfo,
    ) -> Option<String> {
        type_info
            .base_type
            .strip_suffix("Class")
            .map(|base_name| format!("{}Props", base_name))
    }

    /// Find which class owns this property enum by tracing N_PROPS through
    /// GParamSpec arrays or by finding install_property calls
    /// Returns the class type and associated get/set property function names
    fn find_class_context_for_enum(
        &self,
        file: &gobject_ast::FileModel,
        enum_info: &gobject_ast::EnumInfo,
    ) -> Option<ClassContext> {
        // Get N_PROPS name if present
        let n_props_name = enum_info
            .values
            .last()
            .filter(|v| v.is_prop_last())
            .map(|v| v.name.as_str());

        // Get all property enum value names (excluding PROP_0 and sentinels)
        let property_names: Vec<&str> = enum_info
            .values
            .iter()
            .filter(|v| !v.is_prop_0() && !v.is_prop_last())
            .map(|v| v.name.as_str())
            .collect();

        // Step 1: Find GParamSpec array declarations that use N_PROPS
        let array_names = if let Some(sentinel) = n_props_name {
            self.find_param_spec_arrays_for_sentinel(file, sentinel)
        } else {
            Vec::new()
        };

        // Step 2: Find class_init function that uses this array OR uses
        // install_property
        for func in file.iter_function_definitions() {
            if !func.name.ends_with("_class_init") {
                continue;
            }

            let mut uses_property_enum = false;

            // Check if this class_init uses the array (install_properties path)
            if !array_names.is_empty() {
                uses_property_enum = func.body_statements.iter().any(|stmt| {
                    let mut found = false;
                    stmt.walk(&mut |s| {
                        if let gobject_ast::Statement::Expression(expr_stmt) = s {
                            match &expr_stmt.expr {
                                // Check assignments: my_props[PROP_NAME] = ...
                                gobject_ast::Expression::Assignment(assignment) => {
                                    if let gobject_ast::Expression::Subscript(subscript) =
                                        &*assignment.lhs
                                        && let gobject_ast::Expression::Identifier(id) =
                                            &*subscript.array
                                        && array_names.contains(&id.name.as_str())
                                    {
                                        found = true;
                                    }
                                }
                                // Check calls: g_object_class_install_properties(..., my_props)
                                gobject_ast::Expression::Call(call)
                                    if call.function_contains("install_properties") =>
                                {
                                    for arg in &call.arguments {
                                        let gobject_ast::Argument::Expression(expr) = arg;
                                        if let gobject_ast::Expression::Identifier(ident) =
                                            expr.as_ref()
                                        {
                                            for array_name in &array_names {
                                                if &ident.name == array_name {
                                                    found = true;
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    });
                    found
                });
            }

            // If no array usage found, check for install_property (singular) calls
            // that use any of our property enum values
            if !uses_property_enum && !property_names.is_empty() {
                func.body_statements.iter().for_each(|stmt| {
                    stmt.walk(&mut |s| {
                        if let gobject_ast::Statement::Expression(expr_stmt) = s
                            && let gobject_ast::Expression::Call(call) = &expr_stmt.expr
                            && call.function_contains("install_property")
                            && !call.function_contains("install_properties")
                        {
                            // Check if any argument uses our property enum values
                            for arg in &call.arguments {
                                if let gobject_ast::Argument::Expression(expr) = arg
                                    && let gobject_ast::Expression::Identifier(ident) =
                                        expr.as_ref()
                                    && property_names.contains(&ident.name.as_str())
                                {
                                    uses_property_enum = true;
                                    break;
                                }
                            }
                        }
                    });
                });
            }

            if !uses_property_enum {
                continue;
            }

            // Extract class type from parameter
            let class_type_info = func.parameters.first().map(|p| p.type_info.clone())?;

            // Extract get_property and set_property function names from assignments
            let mut get_property_func = None;
            let mut set_property_func = None;

            for stmt in &func.body_statements {
                stmt.walk(&mut |s| {
                    if let gobject_ast::Statement::Expression(expr_stmt) = s
                        && let gobject_ast::Expression::Assignment(assignment) = &expr_stmt.expr
                        && let gobject_ast::Expression::FieldAccess(field) = &*assignment.lhs
                        && let gobject_ast::Expression::Identifier(ident) = assignment.rhs.as_ref()
                    {
                        // Check for object_class->get_property = func_name
                        if field.field == "get_property" {
                            get_property_func = Some(ident.name.to_string());
                        } else if field.field == "set_property" {
                            set_property_func = Some(ident.name.to_string());
                        }
                    }
                });
            }

            return Some(ClassContext {
                class_type_info,
                get_property_func,
                set_property_func,
            });
        }

        None
    }

    /// Find GParamSpec array names that use the given sentinel
    fn find_param_spec_arrays_for_sentinel<'a>(
        &self,
        file: &'a gobject_ast::FileModel,
        sentinel_name: &str,
    ) -> Vec<&'a str> {
        let mut array_names = Vec::new();

        for item in &file.top_level_items {
            self.find_param_spec_arrays_in_item_for_sentinel(item, sentinel_name, &mut array_names);
        }

        array_names
    }

    fn find_param_spec_arrays_in_item_for_sentinel<'a>(
        &self,
        item: &'a gobject_ast::top_level::TopLevelItem,
        sentinel_name: &str,
        array_names: &mut Vec<&'a str>,
    ) {
        use gobject_ast::{
            Statement,
            top_level::{PreprocessorDirective, TopLevelItem},
        };

        match item {
            TopLevelItem::Declaration(Statement::Declaration(decl))
                if decl.type_info.is_base_type("GParamSpec") && decl.type_info.is_pointer() =>
            {
                // Check if it's an array declaration using the sentinel name
                if let Some(Expression::Identifier(size_id)) = &decl.array_size
                    && size_id.name == sentinel_name
                {
                    array_names.push(&decl.name);
                }
            }
            TopLevelItem::Preprocessor(PreprocessorDirective::Conditional { body, .. }) => {
                for nested_item in body {
                    self.find_param_spec_arrays_in_item_for_sentinel(
                        nested_item,
                        sentinel_name,
                        array_names,
                    );
                }
            }
            _ => {}
        }
    }

    /// Check if N_PROPS is used in switch case expressions in
    /// get_property/set_property e.g., case N_PROPS +
    /// META_DBUS_SESSION_PROP_FOO:
    fn n_props_used_in_switch_cases(
        &self,
        file: &gobject_ast::FileModel,
        n_props_name: &str,
    ) -> bool {
        for func in file.iter_function_definitions() {
            if !func.name.ends_with("_get_property") && !func.name.ends_with("_set_property") {
                continue;
            }

            // Check all switch statements in the function
            for stmt in &func.body_statements {
                for switch_stmt in stmt.iter_switches() {
                    // Check if any case label uses n_props_name
                    for case in &switch_stmt.cases {
                        if let Some(value_expr) = &case.value
                            && self.expression_uses_identifier(value_expr, n_props_name)
                        {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Check if an expression uses a specific identifier
    fn expression_uses_identifier(&self, expr: &gobject_ast::Expression, identifier: &str) -> bool {
        use gobject_ast::Expression;
        match expr {
            Expression::Identifier(ident) => ident.name == identifier,
            Expression::Binary(binary) => {
                self.expression_uses_identifier(&binary.left, identifier)
                    || self.expression_uses_identifier(&binary.right, identifier)
            }
            Expression::Unary(unary) => self.expression_uses_identifier(&unary.operand, identifier),
            Expression::Call(call) => call.arguments.iter().any(|arg| {
                let gobject_ast::Argument::Expression(expr) = arg;
                self.expression_uses_identifier(expr, identifier)
            }),
            Expression::Cast(cast) => self.expression_uses_identifier(&cast.operand, identifier),
            Expression::Conditional(cond) => {
                self.expression_uses_identifier(&cond.condition, identifier)
                    || self.expression_uses_identifier(&cond.then_expr, identifier)
                    || self.expression_uses_identifier(&cond.else_expr, identifier)
            }
            Expression::FieldAccess(_) => {
                // FieldAccessExpression only stores text, not parsed sub-expressions
                false
            }
            Expression::Subscript(sub) => {
                self.expression_uses_identifier(&sub.array, identifier)
                    || self.expression_uses_identifier(&sub.index, identifier)
            }
            _ => false,
        }
    }

    /// Build a map of enum value name -> whether it's an override property
    /// by parsing all property installations in class_init
    fn build_property_override_map(
        &self,
        file: &gobject_ast::FileModel,
    ) -> std::collections::HashMap<String, bool> {
        use gobject_ast::Expression;

        let mut property_map = std::collections::HashMap::new();

        // Find class_init function
        for func in file.iter_function_definitions() {
            if !func.name.ends_with("_class_init") {
                continue;
            }

            // Look for property installations
            for stmt in &func.body_statements {
                stmt.walk(&mut |s| {
                    if let gobject_ast::Statement::Expression(expr_stmt) = s {
                        match &expr_stmt.expr {
                            // Array assignment: obj_props[PROP_NAME] = g_param_spec_*(...)
                            Expression::Assignment(assignment) => {
                                if let Expression::Subscript(subscript) = &*assignment.lhs
                                    && let Expression::Identifier(enum_id) = &*subscript.index
                                    && let Expression::Call(call) = &*assignment.rhs
                                {
                                    // Check if this is a g_param_spec_* call
                                    if let Some(prop) =
                                        gobject_ast::Property::from_param_spec_call(call)
                                    {
                                        let is_override = matches!(
                                            prop.property_type,
                                            gobject_ast::PropertyType::Override
                                        );
                                        property_map.insert(enum_id.name.clone(), is_override);
                                    }
                                }
                            }
                            // Direct call: g_object_class_override_property(class,
                            // PROP_ORIENTATION, "orientation")
                            Expression::Call(call) => {
                                if let Some(prop) =
                                    gobject_ast::Property::from_override_property_call(call)
                                {
                                    // Extract the enum value (second argument)
                                    if let Some(arg) = call.get_arg(1)
                                        && let Expression::Identifier(id) = arg
                                    {
                                        let is_override = matches!(
                                            prop.property_type,
                                            gobject_ast::PropertyType::Override
                                        );
                                        property_map.insert(id.name.clone(), is_override);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                });
            }
        }

        property_map
    }
}
