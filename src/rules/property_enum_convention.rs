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
                .iter_property_enums()
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

            for enum_info in file.iter_property_enums() {
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

                // Find which class_init and property functions correspond to this enum
                // by tracing N_PROPS through GParamSpec array to class_init
                let (class_context, assignments) =
                    match self.find_class_context_for_enum(file, enum_info) {
                        Some(pair) => pair,
                        None => continue,
                    };

                // Build map of which properties are overrides from this class_init's
                // assignments
                let property_map = self.build_property_override_map(&assignments);

                // Get the name of the last REAL property (the one before N_PROPS)
                // If N_PROPS = PROP_X and PROP_X is an override, find the last non-override
                // property
                let last_real_prop_name = if has_n_props && enum_info.values.len() >= 2 {
                    // Check if the value before N_PROPS is an override
                    let second_to_last = &enum_info.values[enum_info.values.len() - 2];

                    // Also check if N_PROPS = PROP_X where PROP_X is an override
                    let n_props_value = enum_info.values.last().unwrap();
                    let n_props_points_to_override = if n_props_value.value_location.is_some()
                        && n_props_value.value.is_none()
                    {
                        n_props_value
                            .value_text(&file.source)
                            .and_then(|value_text| property_map.get(value_text).copied())
                            .unwrap_or(false)
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

                // Determine the enum name to use (either from typedef or derived from
                // class_init)
                let derived_enum_name = if enum_info.name.is_none() {
                    self.derive_enum_name_from_class_type(&class_context.class_type_info)
                } else {
                    None
                };

                let mut fixes = Vec::new();

                // Fix 0: Convert anonymous enum to typedef if needed
                if let Some(ref enum_name) = derived_enum_name {
                    fixes.extend(self.create_typedef_fixes(file, enum_info, enum_name));
                }

                // Fix 1: Remove PROP_0 line entirely (including any blank line after it)
                if has_prop_0 && enum_info.values.len() >= 2 {
                    let prop_0 = &enum_info.values[0];

                    // Use SourceLocation's find_line_bounds_with_following_blank
                    let (line_start, line_end) = prop_0
                        .location
                        .find_line_bounds_with_following_blank(&file.source);
                    fixes.push(Fix::new(line_start, line_end, String::new()));
                }

                // Fix 2: Add " = 1" to the first real property (second value)
                if has_prop_0 && enum_info.values.len() >= 2 {
                    let first_real = &enum_info.values[1];

                    // If the property already has a value (e.g., "= 0"), remove it first
                    if first_real.value == Some(0)
                        && let Some(value_loc) = &first_real.value_location
                    {
                        // Remove existing " = 0" or "= 0" and replace with " = 1"
                        fixes.push(Fix::new(
                            first_real.name_location.end_byte,
                            value_loc.end_byte,
                            " = 1".to_string(),
                        ));
                    } else {
                        // Just insert " = 1" right after the property name
                        fixes.push(Fix::new(
                            first_real.name_location.end_byte,
                            first_real.name_location.end_byte,
                            " = 1".to_string(),
                        ));
                    }
                }

                // Fix 3: Remove N_PROPS line entirely (including any blank line before it)
                if has_n_props && enum_info.values.len() >= 2 {
                    let n_props = enum_info.values.last().unwrap();

                    // Use SourceLocation's find_line_bounds which already handles preceding blank
                    // lines
                    let (line_start, line_end) = n_props.location.find_line_bounds(&file.source);
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
                    for func in file.iter_class_init_functions() {
                        for call in func.find_install_properties_calls() {
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
                // Only apply to the specific property functions for this enum
                let enum_name = if let Some(ref name) = enum_info.name {
                    name.clone()
                } else if let Some(ref derived) = derived_enum_name {
                    derived.clone()
                } else {
                    // Try to derive from class type if we have it
                    self.derive_enum_name_from_class_type(&class_context.class_type_info)
                        .unwrap_or_else(|| "UnknownProps".to_string())
                };

                if !enum_name.is_empty() {
                    if let Some(ref func_name) = class_context.get_property_func {
                        self.add_switch_cast_for_function(file, func_name, &enum_name, &mut fixes);
                    }
                    if let Some(ref func_name) = class_context.set_property_func {
                        self.add_switch_cast_for_function(file, func_name, &enum_name, &mut fixes);
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

            // Check modern enums (without PROP_0/N_PROPS) for outdated array sizes
            // and missing switch casts
            for enum_info in file.iter_property_enums() {
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

                // Skip old-style enums (already handled above)
                if has_prop_0 || has_n_props {
                    continue;
                }

                // Only check already-modern enums (ones with explicit = 1 on first value)
                let is_already_modern = enum_info
                    .values
                    .first()
                    .and_then(|v| v.value.as_ref())
                    .is_some_and(|val| *val == 1);

                if !is_already_modern {
                    continue;
                }

                let (class_context, assignments) =
                    match self.find_class_context_for_enum(file, enum_info) {
                        Some(pair) => pair,
                        None => continue,
                    };

                let property_map = self.build_property_override_map(&assignments);

                // Find the last real (non-override) property
                let last_real_prop = enum_info
                    .values
                    .iter()
                    .rev()
                    .find(|v| !property_map.get(&v.name).copied().unwrap_or(false));

                let Some(last_real_prop) = last_real_prop else {
                    continue;
                };

                // Check GParamSpec arrays for outdated PROP_X + 1 pattern
                self.check_outdated_array_sizes(
                    file,
                    path,
                    enum_info,
                    &last_real_prop.name,
                    violations,
                );

                // Check if getter/setter need switch casts or typedef
                self.check_modern_enum_switch_casts(
                    file,
                    path,
                    enum_info,
                    &class_context,
                    violations,
                );
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
    ) -> Option<(ClassContext, Vec<gobject_ast::ParamSpecAssignment>)> {
        let (func, assignments) = file.find_class_init_for_property_enum(enum_info)?;

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

        Some((
            ClassContext {
                class_type_info,
                get_property_func,
                set_property_func,
            },
            assignments,
        ))
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
                        if let Some(value_expr) = &case.label.value
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
    /// from the given param_spec assignments
    fn build_property_override_map(
        &self,
        assignments: &[gobject_ast::ParamSpecAssignment],
    ) -> std::collections::HashMap<String, bool> {
        use gobject_ast::PropertyType;

        let mut property_map = std::collections::HashMap::new();

        for assignment in assignments {
            // Only track assignments that have an enum_value (ArraySubscript and
            // OverrideProperty)
            if let Some(enum_value) = assignment.enum_value() {
                let is_override =
                    matches!(assignment.property().property_type, PropertyType::Override);
                property_map.insert(enum_value.to_string(), is_override);
            }
        }

        property_map
    }

    /// Check if modern enum needs switch casts in getter/setter (and typedef if
    /// anonymous)
    fn check_modern_enum_switch_casts(
        &self,
        file: &gobject_ast::FileModel,
        path: &std::path::Path,
        enum_info: &gobject_ast::EnumInfo,
        class_context: &ClassContext,
        violations: &mut Vec<Violation>,
    ) {
        // Determine the enum name (derive if anonymous)
        let enum_name = if let Some(ref name) = enum_info.name {
            name.clone()
        } else {
            // For anonymous enums, derive the name from the class type
            match self.derive_enum_name_from_class_type(&class_context.class_type_info) {
                Some(name) => name,
                None => return, // Can't derive a name
            }
        };

        let mut fixes = Vec::new();

        // If anonymous enum, add typedef
        if enum_info.name.is_none() {
            fixes.extend(self.create_typedef_fixes(file, enum_info, &enum_name));
        }

        // Add switch casts for getter/setter functions
        if let Some(ref func_name) = class_context.get_property_func {
            self.add_switch_cast_for_function(file, func_name, &enum_name, &mut fixes);
        }
        if let Some(ref func_name) = class_context.set_property_func {
            self.add_switch_cast_for_function(file, func_name, &enum_name, &mut fixes);
        }

        // Only create a violation if we actually need to add fixes
        if !fixes.is_empty() {
            let message = if enum_info.name.is_none() {
                format!(
                    "Add typedef {} and use cast in switch statements for type safety",
                    enum_name
                )
            } else {
                format!(
                    "Add ({}) cast to switch statements for type safety",
                    enum_name
                )
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

    /// Create fixes to convert an anonymous enum to a typedef enum
    fn create_typedef_fixes(
        &self,
        file: &gobject_ast::FileModel,
        enum_info: &gobject_ast::EnumInfo,
        enum_name: &str,
    ) -> Vec<Fix> {
        let mut fixes = Vec::new();

        // Add "typedef " before "enum"
        fixes.push(Fix::new(
            enum_info.location.start_byte,
            enum_info.location.start_byte,
            "typedef ".to_string(),
        ));

        // Add enum name and semicolon after the closing brace
        let mut semicolon_pos = enum_info.body_location.end_byte;
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

        fixes
    }

    /// Check for GParamSpec arrays with outdated PROP_X + 1 sizes
    fn check_outdated_array_sizes(
        &self,
        file: &gobject_ast::FileModel,
        path: &std::path::Path,
        enum_info: &gobject_ast::EnumInfo,
        expected_last_prop: &str,
        violations: &mut Vec<Violation>,
    ) {
        // Build set of property names from this enum
        let property_names: std::collections::HashSet<&str> =
            enum_info.values.iter().map(|v| v.name.as_str()).collect();

        // Find all GParamSpec pointer arrays
        let arrays = file.find_typed_arrays("GParamSpec", true, None);

        for decl in arrays {
            // Check for PROP_X + 1 pattern
            if let Some(Expression::Binary(binary)) = &decl.array_size
                && let Expression::Identifier(prop_id) = &*binary.left
                && property_names.contains(prop_id.name.as_str())
            {
                // This array uses a property from our enum
                // Check if this property is outdated (not the expected last property)
                if prop_id.name != expected_last_prop {
                    let replacement = format!("{} + 1", expected_last_prop);
                    let fix = Fix::new(
                        binary.location.start_byte,
                        binary.location.end_byte,
                        replacement,
                    );

                    violations.push(self.violation_with_fixes(
                        path,
                        binary.location.line,
                        binary.location.column,
                        format!(
                            "GParamSpec array size uses outdated property (should be {} + 1)",
                            expected_last_prop
                        ),
                        vec![fix],
                    ));
                }
            }
        }
    }
}
