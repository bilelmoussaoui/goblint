use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct PropertyEnumConvention;

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
                .filter(|e| self.is_property_enum(e))
                .filter(|e| {
                    // Apply same checks as main loop to see if this enum will be transformed
                    let has_prop_0 = e
                        .values
                        .first()
                        .map(|v| v.name.ends_with("_PROP_0") || v.name == "PROP_0")
                        .unwrap_or(false);

                    let has_n_props_at_end = e
                        .values
                        .last()
                        .map(|v| Self::is_sentinel_name(&v.name))
                        .unwrap_or(false);

                    let has_n_props_in_middle = e.values.iter().enumerate().any(|(idx, v)| {
                        idx < e.values.len() - 1 && Self::is_sentinel_name(&v.name)
                    });

                    // Only count if it will be transformed (not override pattern, not already
                    // modern)
                    !has_n_props_in_middle && (has_prop_0 || has_n_props_at_end)
                })
                .filter_map(|e| {
                    e.values
                        .iter()
                        .find(|v| Self::is_sentinel_name(&v.name))
                        .map(|v| v.name.clone())
                })
                .fold(std::collections::HashMap::new(), |mut map, name| {
                    *map.entry(name).or_insert(0) += 1;
                    map
                });

            for enum_info in file.iter_all_enums() {
                if !self.is_property_enum(enum_info) {
                    continue;
                }

                // Check if this uses the old pattern: PROP_0 at start and N_PROPS at end
                let has_prop_0 = enum_info
                    .values
                    .first()
                    .map(|v| v.name.ends_with("_PROP_0") || v.name == "PROP_0")
                    .unwrap_or(false);

                let has_n_props = enum_info
                    .values
                    .last()
                    .map(|v| Self::is_sentinel_name(&v.name))
                    .unwrap_or(false);

                // Check if N_PROPS appears in the middle (not last) - this is the override
                // properties pattern
                let has_n_props_in_middle = enum_info.values.iter().enumerate().any(|(idx, v)| {
                    idx < enum_info.values.len() - 1 && Self::is_sentinel_name(&v.name)
                });

                if has_n_props_in_middle {
                    // Skip: N_PROPS in the middle means override properties follow
                    // This is a legitimate pattern for interface implementations
                    continue;
                }

                if !has_prop_0 && !has_n_props {
                    // Already using new pattern, skip
                    continue;
                }

                // Get the names we need to work with
                let prop_0_name = enum_info.values.first().unwrap().name.clone();
                let n_props_name = enum_info.values.last().unwrap().name.clone();

                // Get the name of the last REAL property (the one before N_PROPS)
                let last_real_prop_name = if has_n_props && enum_info.values.len() >= 2 {
                    enum_info.values[enum_info.values.len() - 2].name.clone()
                } else {
                    enum_info.values.last().unwrap().name.clone()
                };

                let mut fixes = Vec::new();

                // Fix 1: Remove PROP_0 line entirely
                if has_prop_0 && enum_info.values.len() >= 2 {
                    let prop_0 = &enum_info.values[0];
                    let (line_start, line_end) =
                        self.find_line_bounds(&file.source, prop_0.start_byte, prop_0.end_byte);
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

                // Fix 3: Remove N_PROPS line entirely, and remove trailing comma from previous
                // property
                if has_n_props && enum_info.values.len() >= 2 {
                    let n_props = enum_info.values.last().unwrap();

                    // Remove the N_PROPS line
                    let (line_start, line_end) =
                        self.find_line_bounds(&file.source, n_props.start_byte, n_props.end_byte);
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
    fn is_property_enum(&self, enum_info: &gobject_ast::EnumInfo) -> bool {
        enum_info
            .values
            .iter()
            .any(|v| v.name.contains("_PROP_") || v.name.starts_with("PROP_"))
    }

    /// Check if a name is a sentinel (N_PROPS, PROP_LAST, NUM_PROPERTIES, etc.)
    fn is_sentinel_name(name: &str) -> bool {
        name.ends_with("_N_PROPS")
            || name == "N_PROPS"
            || name.ends_with("_PROP_LAST")
            || name == "PROP_LAST"
            || name.ends_with("_NUM_PROPERTIES")
            || name == "NUM_PROPERTIES"
    }

    /// Find the line bounds (start and end byte positions) for a given byte
    /// range
    fn find_line_bounds(
        &self,
        source: &[u8],
        start_byte: usize,
        end_byte: usize,
    ) -> (usize, usize) {
        // Find the start of the line
        let mut line_start = start_byte;
        while line_start > 0 && source[line_start - 1] != b'\n' {
            line_start -= 1;
        }

        // Find the end of the line (including the newline)
        let mut line_end = end_byte;
        while line_end < source.len() && source[line_end] != b'\n' {
            line_end += 1;
        }
        // Include the newline character
        if line_end < source.len() {
            line_end += 1;
        }

        (line_start, line_end)
    }

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
                &file.source,
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
        source: &[u8],
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
                if ((decl.type_name.contains("GParamSpec") && decl.type_name.contains('*'))
                    || decl.type_name == "GParamSpec*")
                => {
                    // Check if it's an array declaration by looking at the source
                    let decl_text = std::str::from_utf8(
                        &source[decl.location.start_byte..decl.location.end_byte],
                    )
                    .unwrap_or("");

                    // Look for [N_PROPS] in the declaration
                    let pattern = format!("[{}]", n_props_name);
                    if let Some(bracket_pos) = decl_text.find(&pattern) {
                        // Found it! This is a GParamSpec array using N_PROPS
                        let bracket_start = decl.location.start_byte + bracket_pos;
                        let bracket_end = bracket_start + pattern.len();

                        // Fix: Replace [N_PROPS] with [LAST_PROP + 1]
                        let replacement = format!("[{} + 1]", last_prop_name);
                        fixes.push(Fix::new(bracket_start, bracket_end, replacement));

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
                        source,
                        array_names,
                        fixes,
                    );
                }
            }
            _ => {}
        }
    }
}
