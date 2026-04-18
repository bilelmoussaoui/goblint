use std::path::Path;

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct IncludeOrder;

impl Rule for IncludeOrder {
    fn name(&self) -> &'static str {
        "include_order"
    }

    fn description(&self) -> &'static str {
        "Enforce consistent include ordering: config header (configurable), associated header, system headers, project headers"
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
        config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        let config_header = config
            .get_rule_config("include_order")
            .and_then(|rc| rc.options.get("config_header"))
            .and_then(|v| v.as_str())
            .unwrap_or("config.h");

        for (path, file) in ast_context.iter_all_files() {
            self.check_include_groups(
                &file.top_level_items,
                path,
                &file.source,
                config_header,
                violations,
            );
        }
    }
}

impl IncludeOrder {
    /// Check include ordering at each level of the tree structure
    /// All includes at the same level (outside conditionals) are sorted
    /// together
    fn check_include_groups(
        &self,
        items: &[gobject_ast::top_level::TopLevelItem],
        file_path: &Path,
        source: &[u8],
        config_header: &str,
        violations: &mut Vec<Violation>,
    ) {
        use gobject_ast::top_level::{PreprocessorDirective, TopLevelItem};

        // Collect all top-level includes
        let mut top_level_includes = Vec::new();

        for item in items {
            match item {
                TopLevelItem::Preprocessor(PreprocessorDirective::Include {
                    path,
                    is_system,
                    location,
                }) => {
                    top_level_includes.push(gobject_ast::Include {
                        path: path.clone(),
                        is_system: *is_system,
                        location: *location,
                    });
                }
                TopLevelItem::Preprocessor(PreprocessorDirective::Conditional { body, .. }) => {
                    // Recursively check includes within the conditional block
                    self.check_include_groups(body, file_path, source, config_header, violations);
                }
                _ => {}
            }
        }

        // Check and fix all top-level includes as one group
        if !top_level_includes.is_empty() {
            self.check_and_fix_group_scattered(
                &top_level_includes,
                file_path,
                source,
                config_header,
                violations,
            );
        }
    }

    /// Check and fix scattered includes (may be separated by #ifdef blocks)
    /// All includes should be sorted and moved to be consecutive at the start
    fn check_and_fix_group_scattered(
        &self,
        includes: &[gobject_ast::Include],
        file_path: &Path,
        source: &[u8],
        config_header: &str,
        violations: &mut Vec<Violation>,
    ) {
        if includes.is_empty() {
            return;
        }

        // Step 1: Compute expected vs actual order
        let expected_order = self.compute_expected_order(file_path, includes, config_header);
        let current_order: Vec<_> = includes.iter().map(|inc| &inc.path).collect();

        if expected_order == current_order {
            return; // Already in correct order
        }

        // Step 2: Gather information about the include block
        let first_inc = &includes[0];
        let last_inc = includes
            .iter()
            .max_by_key(|inc| inc.location.end_byte)
            .unwrap();

        // Step 3: Determine spacing after the last include (to preserve it)
        // The last include's end_byte points right after its newline
        let pos_after_last = last_inc.location.end_byte;
        let trailing_newlines = {
            let mut count = 0;
            let mut check_pos = pos_after_last;
            while check_pos < source.len() && source[check_pos] == b'\n' {
                count += 1;
                check_pos += 1;
            }
            count
        };

        // Step 4: Generate sorted includes with preserved trailing spacing
        let sorted_text = self.generate_sorted_includes_text(
            &expected_order,
            includes,
            file_path,
            config_header,
            trailing_newlines,
        );

        // Step 5: Build fixes
        let mut fixes = Vec::new();

        // Replace first include with all sorted includes (including trailing newlines)
        // Also consume any blank lines immediately after the first include
        let mut first_end = first_inc.location.end_byte;
        while first_end < source.len() && source[first_end] == b'\n' {
            first_end += 1;
        }
        fixes.push(Fix::new(
            first_inc.location.start_byte,
            first_end,
            sorted_text,
        ));

        // Delete all other includes
        // Consume any blank lines immediately after each deleted include
        for inc in includes.iter().skip(1) {
            let mut end = inc.location.end_byte;
            // Consume blank lines after this include
            while end < source.len() && source[end] == b'\n' {
                end += 1;
            }
            fixes.push(Fix::new(inc.location.start_byte, end, String::new()));
        }

        violations.push(self.violation_with_fixes(
            file_path,
            first_inc.location.line,
            1,
            format!(
                "Includes are not in the correct order. Expected: {} (if present), associated header, standard C headers, system headers (<>), project headers (\"\") (all alphabetically sorted within each group, blank line between groups)",
                config_header
            ),
            fixes,
        ));
    }

    /// Generate the text for sorted includes with proper grouping
    fn generate_sorted_includes_text(
        &self,
        expected_order: &[&str],
        includes: &[gobject_ast::Include],
        file_path: &Path,
        config_header: &str,
        trailing_newlines: usize,
    ) -> String {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum IncludeGroup {
            Config,
            Associated,
            StandardC,
            System,
            Project,
        }

        let grouped_includes: Vec<(&&str, IncludeGroup)> = expected_order
            .iter()
            .map(|path| {
                let group = if *path == config_header {
                    IncludeGroup::Config
                } else if self.is_associated_header(path, file_path) {
                    IncludeGroup::Associated
                } else {
                    let original = includes.iter().find(|inc| inc.path == *path).unwrap();
                    if original.is_system && self.is_standard_c_header(path) {
                        IncludeGroup::StandardC
                    } else if original.is_system {
                        IncludeGroup::System
                    } else {
                        IncludeGroup::Project
                    }
                };
                (path, group)
            })
            .collect();

        let mut result = String::new();
        let mut last_group: Option<IncludeGroup> = None;

        for (path, group) in grouped_includes {
            // Add blank line between groups
            if let Some(prev_group) = last_group
                && prev_group != group
            {
                result.push('\n');
            }
            last_group = Some(group);

            let original = includes.iter().find(|inc| inc.path == *path).unwrap();
            let bracket = if original.is_system {
                ("<", ">")
            } else {
                ("\"", "\"")
            };
            result.push_str(&format!("#include {}{}{}\n", bracket.0, path, bracket.1));
        }

        // Add trailing newlines to preserve original spacing
        for _ in 0..trailing_newlines {
            result.push('\n');
        }

        result
    }

    /// Compute the expected order of includes
    fn compute_expected_order<'a>(
        &self,
        file_path: &Path,
        includes: &'a [gobject_ast::Include],
        config_header: &str,
    ) -> Vec<&'a str> {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        enum IncludeGroup {
            Config = 0,     // config.h must be first
            Associated = 1, // foo.c -> foo.h
            StandardC = 2,  // <stdio.h>, <math.h>, etc.
            System = 3,     // <glib.h>, <gtk/gtk.h>, etc.
            Project = 4,    // "..."
        }

        let mut grouped: Vec<(&gobject_ast::Include, IncludeGroup)> = includes
            .iter()
            .map(|inc| {
                let group = if inc.path == config_header {
                    IncludeGroup::Config
                } else if self.is_associated_header(&inc.path, file_path) {
                    IncludeGroup::Associated
                } else if inc.is_system && self.is_standard_c_header(&inc.path) {
                    IncludeGroup::StandardC
                } else if inc.is_system {
                    IncludeGroup::System
                } else {
                    IncludeGroup::Project
                };
                (inc, group)
            })
            .collect();

        // Sort by group first, then alphabetically within each group
        grouped.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.path.cmp(&b.0.path)));

        grouped.iter().map(|(inc, _)| inc.path.as_str()).collect()
    }

    /// Check if a header is a standard C library header
    fn is_standard_c_header(&self, path: &str) -> bool {
        matches!(
            path,
            "assert.h"
                | "complex.h"
                | "ctype.h"
                | "errno.h"
                | "fenv.h"
                | "float.h"
                | "inttypes.h"
                | "iso646.h"
                | "limits.h"
                | "locale.h"
                | "math.h"
                | "setjmp.h"
                | "signal.h"
                | "stdalign.h"
                | "stdarg.h"
                | "stdatomic.h"
                | "stdbool.h"
                | "stddef.h"
                | "stdint.h"
                | "stdio.h"
                | "stdlib.h"
                | "stdnoreturn.h"
                | "string.h"
                | "tgmath.h"
                | "threads.h"
                | "time.h"
                | "uchar.h"
                | "wchar.h"
                | "wctype.h"
        )
    }

    /// Get all possible associated headers for a C file
    /// Returns the basenames to check (without directory prefix)
    /// foo.c -> ["foo.h", "foo-private.h", "fooprivate.h"]
    fn get_associated_header_basenames(&self, file_path: &Path) -> Vec<String> {
        if file_path.extension() != Some(std::ffi::OsStr::new("c")) {
            return Vec::new();
        }

        let Some(stem) = file_path.file_stem().and_then(|s| s.to_str()) else {
            return Vec::new();
        };

        vec![
            format!("{}.h", stem),         // foo.c -> foo.h
            format!("{}-private.h", stem), // foo.c -> foo-private.h
            format!("{}private.h", stem),  // foo.c -> fooprivate.h
        ]
    }

    /// Check if an include path is an associated header for the given file
    /// Checks the basename of the include, so "meta-test/meta-test-monitor.h"
    /// matches for "meta-test-monitor.c"
    fn is_associated_header(&self, include_path: &str, file_path: &Path) -> bool {
        let basenames = self.get_associated_header_basenames(file_path);

        // Extract basename from include path (part after last '/')
        let include_basename = include_path.rsplit('/').next().unwrap_or(include_path);

        basenames.iter().any(|pattern| pattern == include_basename)
    }
}
