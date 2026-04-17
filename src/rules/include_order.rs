use std::path::Path;

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct IncludeOrder;

impl Rule for IncludeOrder {
    fn name(&self) -> &'static str {
        "include_order"
    }

    fn description(&self) -> &'static str {
        "Enforce consistent include ordering: config.h, associated header, system headers, project headers"
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
            self.check_include_groups(&file.top_level_items, path, &file.source, violations);
        }
    }
}

impl IncludeOrder {
    /// Check include ordering at each level of the tree structure
    /// All includes at the same level (outside conditionals) are sorted together
    fn check_include_groups(
        &self,
        items: &[gobject_ast::top_level::TopLevelItem],
        file_path: &Path,
        source: &[u8],
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
                    self.check_include_groups(body, file_path, source, violations);
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
        violations: &mut Vec<Violation>,
    ) {
        if includes.is_empty() {
            return;
        }

        let expected_order = self.compute_expected_order(file_path, includes);
        let current_order: Vec<_> = includes.iter().map(|inc| &inc.path).collect();

        if expected_order != current_order {
            let mut fixes = Vec::new();

            // Check what comes after the LAST include in source order
            // (this is what will come after the sorted includes after deletions)
            let last_inc = &includes[includes.len() - 1];
            let mut pos = last_inc.location.end_byte;

            // Skip the newline at end of include line
            if pos < source.len() && source[pos] == b'\n' {
                pos += 1;
            }

            // Check if next line is already blank or is a preprocessor directive
            let next_is_blank_or_preprocessor =
                (pos < source.len() && source[pos] == b'\n') ||
                (pos < source.len() && source[pos] == b'#');
            let skip_trailing_newline = next_is_blank_or_preprocessor;

            // Generate sorted includes text
            let sorted_text =
                self.generate_sorted_includes_text(&expected_order, includes, file_path, skip_trailing_newline);

            // Check if there's a blank line after the first include that we need to consume
            let first_inc = &includes[0];
            let mut first_end = first_inc.location.end_byte;
            if first_end < source.len() && source[first_end] == b'\n' {
                first_end += 1;
                // If there's a blank line, include it in the replacement
                if first_end < source.len() && source[first_end] == b'\n' {
                    first_end += 1;
                }
            }

            // Replace first include (and any blank line after it) with all sorted includes
            fixes.push(Fix::new(
                includes[0].location.start_byte,
                first_end,
                sorted_text,
            ));

            // Remove all other includes
            for inc in &includes[1..] {
                let mut end_byte = inc.location.end_byte;
                // Remove the include line and its newline
                if end_byte < source.len() && source[end_byte] == b'\n' {
                    end_byte += 1;
                }
                fixes.push(Fix::new(inc.location.start_byte, end_byte, String::new()));
            }

            violations.push(self.violation_with_fixes(
                file_path,
                includes[0].location.line,
                1,
                "Includes are not in the correct order. Expected: config.h (if present), associated header, standard C headers, system headers (<>), project headers (\"\") (all alphabetically sorted within each group, blank line between groups)".to_string(),
                fixes,
            ));
        }
    }

    /// Generate the text for sorted includes with proper grouping
    fn generate_sorted_includes_text(
        &self,
        expected_order: &[&str],
        includes: &[gobject_ast::Include],
        file_path: &Path,
        skip_trailing_newline: bool,
    ) -> String {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum IncludeGroup {
            Config,
            Associated,
            StandardC,
            System,
            Project,
        }

        let associated_headers = self.get_associated_headers(file_path);

        let grouped_includes: Vec<(&&str, IncludeGroup)> = expected_order
            .iter()
            .map(|path| {
                let group = if *path == "config.h" {
                    IncludeGroup::Config
                } else if associated_headers.contains(&path.to_string()) {
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

        // Add trailing blank line unless we should skip it
        if !skip_trailing_newline {
            result.push('\n');
        }

        result
    }

    /// Compute the expected order of includes
    fn compute_expected_order<'a>(
        &self,
        file_path: &Path,
        includes: &'a [gobject_ast::Include],
    ) -> Vec<&'a str> {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        enum IncludeGroup {
            Config = 0,     // config.h must be first
            Associated = 1, // foo.c -> foo.h
            StandardC = 2,  // <stdio.h>, <math.h>, etc.
            System = 3,     // <glib.h>, <gtk/gtk.h>, etc.
            Project = 4,    // "..."
        }

        let associated_headers = self.get_associated_headers(file_path);

        let mut grouped: Vec<(&gobject_ast::Include, IncludeGroup)> = includes
            .iter()
            .map(|inc| {
                let group = if inc.path == "config.h" {
                    IncludeGroup::Config
                } else if associated_headers.contains(&inc.path) {
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
    /// foo.c -> ["foo.h", "foo-private.h", "fooprivate.h"]
    /// wayland/foo.c -> ["foo.h", "wayland/foo.h", "foo-private.h",
    /// "wayland/foo-private.h", ...]
    fn get_associated_headers(&self, file_path: &Path) -> Vec<String> {
        if file_path.extension() != Some(std::ffi::OsStr::new("c")) {
            return Vec::new();
        }

        let Some(stem) = file_path.file_stem().and_then(|s| s.to_str()) else {
            return Vec::new();
        };

        let mut headers = Vec::new();

        // Common patterns for associated headers (basename only)
        let base_patterns = vec![
            format!("{}.h", stem),         // foo.c -> foo.h
            format!("{}-private.h", stem), // foo.c -> foo-private.h
            format!("{}private.h", stem),  // foo.c -> fooprivate.h
        ];

        // Add basename patterns
        headers.extend(base_patterns.iter().cloned());

        // Also add patterns with parent directory prefix if present
        // e.g., src/wayland/foo.c -> wayland/foo.h
        if let Some(parent) = file_path.parent()
            && let Some(parent_name) = parent.file_name().and_then(|s| s.to_str())
        {
            for pattern in &base_patterns {
                headers.push(format!("{}/{}", parent_name, pattern));
            }
        }

        headers
    }
}
