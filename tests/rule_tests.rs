use std::{fs, path::Path};

use globset::GlobSetBuilder;
use goblint::{ast_context::AstContext, config::Config, fixer, rules::Rule};

/// Build an AstContext from a single C file copied into a temp directory.
/// Also copies any sibling .h files from the fixture directory.
/// Returns the TempDir (must stay alive for the duration of the test).
fn build_context_for_file(test_file: &Path) -> (AstContext, tempfile::TempDir) {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let dest = temp_dir.path().join(test_file.file_name().unwrap());
    fs::copy(test_file, &dest).expect("failed to copy fixture");

    // Also copy any .h files from the same directory (for rules that inspect
    // headers)
    if let Some(fixture_dir) = test_file.parent()
        && let Ok(entries) = fs::read_dir(fixture_dir)
    {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "h") {
                let h_dest = temp_dir.path().join(path.file_name().unwrap());
                fs::copy(&path, &h_dest).expect("failed to copy header fixture");
            }
        }
    }

    let ignore = GlobSetBuilder::new().build().unwrap();
    let ctx = AstContext::build_with_ignore(temp_dir.path(), &ignore, None)
        .expect("failed to build AstContext");

    (ctx, temp_dir)
}

/// Format violations as `filename:line:col: rule: message`, sorted.
fn format_violations(violations: &[goblint::rules::Violation], strip_prefix: &Path) -> String {
    let lines: Vec<String> = violations
        .iter()
        .map(|v| {
            let relative = v.file.strip_prefix(strip_prefix).unwrap_or(&v.file);
            format!(
                "{}:{}:{}: {}: {}",
                relative.display(),
                v.line,
                v.column,
                v.rule,
                v.message
            )
        })
        .collect();
    lines.join("\n")
}

/// Core fixture runner for a single rule.
///
/// - Iterates all `*.c` files in `tests/fixtures/<rule_name>/`
/// - Runs the rule, compares violations against `<stem>.stderr`
/// - If `<stem>.fixed.c` exists, applies fixes and compares the result
/// - If `<stem>.stderr` doesn't exist or `BLESS=1` is set, writes/updates it
fn run_fixture_tests(rule_name: &str, rule: &dyn Rule) {
    let fixtures_dir = Path::new("tests/fixtures").join(rule_name);
    if !fixtures_dir.exists() {
        return;
    }

    let mut test_files: Vec<_> = fs::read_dir(&fixtures_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            // Test *.c and standalone *.h files (not already tested as part of .c)
            // Exclude *.fixed.{c,h} (those are expected outputs)
            let ext = path.extension();
            let is_c = ext.is_some_and(|e| e == "c");
            let is_standalone_h = ext.is_some_and(|e| e == "h") && {
                // Only include .h if there's no corresponding .c file
                let stem = path.file_stem().unwrap();
                let c_file = path.with_file_name(stem).with_extension("c");
                !c_file.exists()
            };

            (is_c || is_standalone_h)
                && !path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.ends_with(".fixed"))
        })
        .map(|e| e.path())
        .collect();
    test_files.sort();

    let bless = std::env::var("BLESS").is_ok();
    let mut failures: Vec<String> = Vec::new();

    for test_file in test_files {
        let stem = test_file.file_stem().unwrap().to_str().unwrap().to_owned();
        let ext = test_file.extension().unwrap().to_str().unwrap();
        let stderr_file = fixtures_dir.join(format!("{stem}.stderr"));
        let fixed_file = fixtures_dir.join(format!("{stem}.fixed.{ext}"));

        // --- violation check ---
        let (ctx, temp_dir) = build_context_for_file(&test_file);
        let config = Config::default();

        let mut violations = Vec::new();
        rule.check_all(&ctx, &config, &mut violations);
        violations.sort_by_key(|v| (v.line, v.column));

        let actual_stderr = format_violations(&violations, temp_dir.path());

        if bless || !stderr_file.exists() {
            fs::write(&stderr_file, format!("{actual_stderr}\n")).expect("failed to write .stderr");
            if bless {
                println!("blessed {}", stderr_file.display());
            }
        } else {
            let expected = fs::read_to_string(&stderr_file).unwrap_or_default();
            if actual_stderr.trim() != expected.trim() {
                // Write both to temp files for diff
                let expected_path = temp_dir.path().join("expected.stderr");
                let actual_path = temp_dir.path().join("actual.stderr");
                fs::write(&expected_path, &expected).expect("failed to write expected");
                fs::write(&actual_path, &actual_stderr).expect("failed to write actual");

                // Run diff to show the differences
                let diff_output = std::process::Command::new("diff")
                    .arg("-u")
                    .arg("--label")
                    .arg("expected")
                    .arg("--label")
                    .arg("actual")
                    .arg(&expected_path)
                    .arg(&actual_path)
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                    .unwrap_or_else(|_| {
                        format!(
                            "Failed to run diff\n--- expected ---\n{}\n--- got ---\n{}",
                            expected.trim(),
                            actual_stderr.trim()
                        )
                    });

                failures.push(format!(
                    "fixture {rule_name}/{stem}: violations mismatch\n{}",
                    diff_output
                ));
            }
        }

        // --- fix check ---
        if fixed_file.exists() {
            fixer::apply_fixes(&violations).expect("failed to apply fixes");

            let temp_c = temp_dir.path().join(test_file.file_name().unwrap());
            let actual_fixed = fs::read_to_string(&temp_c).expect("failed to read fixed file");
            let expected_fixed = fs::read_to_string(&fixed_file).expect("failed to read .fixed.c");

            if actual_fixed != expected_fixed {
                // Write both to temp files for diff
                let expected_path = temp_dir.path().join("expected.c");
                let actual_path = temp_dir.path().join("actual.c");
                fs::write(&expected_path, &expected_fixed).expect("failed to write expected");
                fs::write(&actual_path, &actual_fixed).expect("failed to write actual");

                // Run diff to show the differences
                let diff_output = std::process::Command::new("diff")
                    .arg("-u")
                    .arg("--label")
                    .arg("expected")
                    .arg("--label")
                    .arg("actual")
                    .arg(&expected_path)
                    .arg(&actual_path)
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                    .unwrap_or_else(|_| {
                        format!(
                            "Failed to run diff\n--- expected ---\n{}\n--- got ---\n{}",
                            expected_fixed.trim(),
                            actual_fixed.trim()
                        )
                    });

                failures.push(format!(
                    "fixture {rule_name}/{stem}: fix output mismatch\n{}",
                    diff_output
                ));
            }

            // --- post-fix violation check ---
            // Re-run the rule on the fixed file to verify which violations remain.
            let fixed_stderr_file = fixtures_dir.join(format!("{stem}.fixed.stderr"));
            let (ctx_fixed, temp_dir_fixed) = build_context_for_file(&temp_c);
            let mut post_fix_violations = Vec::new();
            rule.check_all(&ctx_fixed, &config, &mut post_fix_violations);
            post_fix_violations.sort_by_key(|v| (v.line, v.column));
            let actual_fixed_stderr =
                format_violations(&post_fix_violations, temp_dir_fixed.path());

            if bless || !fixed_stderr_file.exists() {
                fs::write(&fixed_stderr_file, format!("{actual_fixed_stderr}\n"))
                    .expect("failed to write .fixed.stderr");
                if bless {
                    println!("blessed {}", fixed_stderr_file.display());
                }
            } else {
                let expected = fs::read_to_string(&fixed_stderr_file).unwrap_or_default();
                if actual_fixed_stderr.trim() != expected.trim() {
                    // Write both to temp files for diff
                    let expected_path = temp_dir_fixed.path().join("expected.stderr");
                    let actual_path = temp_dir_fixed.path().join("actual.stderr");
                    fs::write(&expected_path, &expected).expect("failed to write expected");
                    fs::write(&actual_path, &actual_fixed_stderr).expect("failed to write actual");

                    // Run diff to show the differences
                    let diff_output = std::process::Command::new("diff")
                        .arg("-u")
                        .arg("--label")
                        .arg("expected")
                        .arg("--label")
                        .arg("actual")
                        .arg(&expected_path)
                        .arg(&actual_path)
                        .output()
                        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                        .unwrap_or_else(|_| {
                            format!(
                                "Failed to run diff\n--- expected ---\n{}\n--- got ---\n{}",
                                expected.trim(),
                                actual_fixed_stderr.trim()
                            )
                        });

                    failures.push(format!(
                        "fixture {rule_name}/{stem}: post-fix violations mismatch\n{}",
                        diff_output
                    ));
                }
            }
        }
    }

    if !failures.is_empty() {
        panic!("\n{}", failures.join("\n\n"));
    }
}

macro_rules! rule_test {
    ($rule_name:ident, $rule:expr) => {
        #[test]
        fn $rule_name() {
            run_fixture_tests(stringify!($rule_name), &$rule);
        }
    };
}

rule_test!(deprecated_add_private, goblint::rules::DeprecatedAddPrivate);
rule_test!(g_declare_semicolon, goblint::rules::GDeclareSemicolon);
rule_test!(g_error_init, goblint::rules::GErrorInit);
rule_test!(g_error_leak, goblint::rules::GErrorLeak);
rule_test!(g_source_id_not_stored, goblint::rules::GSourceIdNotStored);
rule_test!(
    g_object_virtual_methods_chain_up,
    goblint::rules::GObjectVirtualMethodsChainUp
);
rule_test!(
    g_param_spec_null_nick_blurb,
    goblint::rules::GParamSpecNullNickBlurb
);
rule_test!(
    g_param_spec_static_name_canonical,
    goblint::rules::GParamSpecStaticNameCanonical
);
rule_test!(
    g_param_spec_static_strings,
    goblint::rules::GParamSpecStaticStrings
);
rule_test!(g_task_source_tag, goblint::rules::GTaskSourceTag);
rule_test!(include_order, goblint::rules::IncludeOrder);
rule_test!(
    matching_declare_define,
    goblint::rules::MatchingDeclareDefine
);
rule_test!(
    missing_implementation,
    goblint::rules::MissingImplementation
);
rule_test!(
    property_enum_convention,
    goblint::rules::PropertyEnumConvention
);
rule_test!(property_enum_coverage, goblint::rules::PropertyEnumCoverage);
rule_test!(
    property_switch_exhaustiveness,
    goblint::rules::PropertySwitchExhaustiveness
);
rule_test!(signal_enum_coverage, goblint::rules::SignalEnumCoverage);
rule_test!(
    use_g_object_new_with_properties,
    goblint::rules::UseGObjectNewWithProperties
);
rule_test!(use_g_autofree, goblint::rules::UseGAutofree);
rule_test!(use_g_autolist, goblint::rules::UseGAutolist);
rule_test!(
    use_g_bytes_unref_to_data,
    goblint::rules::UseGBytesUnrefToData
);
rule_test!(use_g_autoptr_error, goblint::rules::UseGAutoptrError);
rule_test!(
    use_g_autoptr_goto_cleanup,
    goblint::rules::UseGAutoptrGotoCleanup
);
rule_test!(
    use_g_autoptr_inline_cleanup,
    goblint::rules::UseGAutoptrInlineCleanup
);
rule_test!(use_g_file_load_bytes, goblint::rules::UseGFileLoadBytes);
rule_test!(use_g_new, goblint::rules::UseGNew);
rule_test!(
    use_g_object_class_install_properties,
    goblint::rules::UseGObjectClassInstallProperties
);
rule_test!(use_g_source_once, goblint::rules::UseGSourceOnce);
rule_test!(
    use_g_clear_signal_handler,
    goblint::rules::UseGClearSignalHandler
);
rule_test!(unnecessary_null_check, goblint::rules::UnnecessaryNullCheck);
rule_test!(use_clear_functions, goblint::rules::UseClearFunctions);
rule_test!(
    use_explicit_default_flags,
    goblint::rules::UseExplicitDefaultFlags
);
rule_test!(use_g_clear_handle_id, goblint::rules::UseGClearHandleId);
rule_test!(use_g_clear_list, goblint::rules::UseGClearList);
rule_test!(
    use_g_clear_weak_pointer,
    goblint::rules::UseGClearWeakPointer
);
rule_test!(
    use_g_object_notify_by_pspec,
    goblint::rules::UseGObjectNotifyByPspec
);
rule_test!(use_g_set_object, goblint::rules::UseGSetObject);
rule_test!(use_g_set_str, goblint::rules::UseGSetStr);
rule_test!(use_g_settings_typed, goblint::rules::UseGSettingsTyped);
rule_test!(use_g_source_constants, goblint::rules::UseGSourceConstants);
rule_test!(use_g_steal_pointer, goblint::rules::UseGStealPointer);
rule_test!(
    use_g_str_has_prefix_suffix,
    goblint::rules::UseGStrHasPrefixSuffix
);
rule_test!(use_g_ascii_functions, goblint::rules::UseGAsciiFunctions);
rule_test!(use_g_strlcpy, goblint::rules::UseGStrlcpy);
rule_test!(
    strcmp_explicit_comparison,
    goblint::rules::StrcmpExplicitComparison
);
rule_test!(use_g_strcmp0, goblint::rules::UseGStrcmp0);
rule_test!(
    use_g_string_free_and_steal,
    goblint::rules::UseGStringFreeAndSteal
);
rule_test!(
    use_g_value_set_static_string,
    goblint::rules::UseGValueSetStaticString
);
rule_test!(use_g_variant_new_typed, goblint::rules::UseGVariantNewTyped);
rule_test!(untranslated_string, goblint::rules::UntranslatedString);
rule_test!(use_pragma_once, goblint::rules::UsePragmaOnce);
