use std::{fs, path::Path};

use globset::GlobSetBuilder;
use goblint::{ast_context::AstContext, config::Config, scanner};

#[test]
fn test_inline_ignore() {
    // Build context for the inline_ignore fixture
    let fixture_dir = Path::new("tests/fixtures/inline_ignore");
    let c_file = fixture_dir.join("test.c");

    // Create temp dir and copy file
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let dest = temp_dir.path().join("test.c");
    fs::copy(&c_file, &dest).expect("failed to copy fixture");

    let ignore = GlobSetBuilder::new().build().unwrap();
    let ctx = AstContext::build_with_ignore(temp_dir.path(), &ignore, None, None)
        .expect("failed to build AstContext");

    // Run scanner
    let mut config = Config::default();
    // Enable only use_g_strlcpy rule for this test
    config
        .enable_only_rules(&["use_g_strlcpy".to_string()])
        .unwrap();

    let violations =
        scanner::scan_with_ast(&ctx, &config, temp_dir.path(), None).expect("failed to scan");

    // Note: This test will also print a warning to stderr about "some_other_rule"
    // being invalid (which is expected behavior - we validate rule names in
    // ignore directives)

    // Format violations
    let actual: Vec<String> = violations
        .iter()
        .map(|v| {
            let relative = v.file.strip_prefix(temp_dir.path()).unwrap_or(&v.file);
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

    // Load expected violations
    let stderr_file = fixture_dir.join("test.stderr");
    let expected = fs::read_to_string(&stderr_file).unwrap_or_default();
    let expected_lines: Vec<&str> = expected.trim().lines().collect();

    // We should only have 2 violations (lines 12 and 27)
    // Lines 16, 20, 24 should be ignored
    assert_eq!(
        violations.len(),
        2,
        "Expected 2 violations, got {}. Violations:\n{}",
        violations.len(),
        actual.join("\n")
    );

    // Check that the violations match expected
    for (i, expected_line) in expected_lines.iter().enumerate() {
        if i >= actual.len() {
            panic!(
                "Missing violation: expected '{}', but only got {} violations",
                expected_line,
                actual.len()
            );
        }
        assert!(
            actual[i].contains(
                &expected_line
                    .split(':')
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(":")
            ),
            "Violation mismatch:\nExpected line to contain: {}\nActual: {}",
            expected_line,
            actual[i]
        );
    }

    println!("✓ Inline ignore test passed - 3 violations were correctly ignored");
}

#[test]
fn test_inline_ignore_wildcards() {
    // Build context for the wildcards fixture
    let fixture_dir = Path::new("tests/fixtures/inline_ignore");
    let c_file = fixture_dir.join("test_wildcards.c");

    // Create temp dir and copy file
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let dest = temp_dir.path().join("test_wildcards.c");
    fs::copy(&c_file, &dest).expect("failed to copy fixture");

    let ignore = GlobSetBuilder::new().build().unwrap();
    let ctx = AstContext::build_with_ignore(temp_dir.path(), &ignore, None, None)
        .expect("failed to build AstContext");

    // Run scanner
    let mut config = Config::default();
    config
        .enable_only_rules(&["use_g_strlcpy".to_string()])
        .unwrap();

    let violations =
        scanner::scan_with_ast(&ctx, &config, temp_dir.path(), None).expect("failed to scan");

    // Both strcpy calls should be ignored by wildcards (all and *)
    assert_eq!(
        violations.len(),
        0,
        "Expected 0 violations with wildcards, got {}",
        violations.len()
    );

    println!("✓ Wildcard ignore test passed - 'all' and '*' wildcards work correctly");
}

#[test]
fn test_inline_ignore_invalid_rule() {
    // Create a test file with an invalid rule name
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let test_file = temp_dir.path().join("test_invalid.c");
    fs::write(
        &test_file,
        r#"#include <string.h>

void test(void) {
    char buf[100];
    /* goblint-ignore-next-line: invalid_rule_name */
    strcpy(buf, "test");
}
"#,
    )
    .expect("failed to write test file");

    let ignore = GlobSetBuilder::new().build().unwrap();
    let ctx = AstContext::build_with_ignore(temp_dir.path(), &ignore, None, None)
        .expect("failed to build AstContext");

    let mut config = Config::default();
    config
        .enable_only_rules(&["use_g_strlcpy".to_string()])
        .unwrap();

    // Note: This will print a warning to stderr about "invalid_rule_name"
    // We can't easily capture stderr in tests, but the warning is printed
    let violations =
        scanner::scan_with_ast(&ctx, &config, temp_dir.path(), None).expect("failed to scan");

    // The violation should NOT be suppressed because the rule name doesn't match
    // (we warn about the invalid name, but don't suppress the actual violation)
    assert_eq!(
        violations.len(),
        1,
        "Expected 1 violation (invalid rule name doesn't suppress), got {}",
        violations.len()
    );

    println!("✓ Invalid rule test passed - warning is printed and violation is not suppressed");
}
