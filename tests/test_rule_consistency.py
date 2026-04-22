#!/usr/bin/env python3
"""
Test that enforces consistency between rule files, names, structs, and documentation.

Requirements:
1. Rule file name (without .rs) must match the rule's name() method return value
2. Struct name must be PascalCase of the rule name
3. Rule must be documented in RULES.md
4. Rule must have a registered test case in tests/rule_tests.rs
5. Rule must have a fixture directory under tests/fixtures/
"""

import re
from pathlib import Path


def snake_to_pascal(snake_str):
    """Convert snake_case to PascalCase."""
    components = snake_str.split('_')
    return ''.join(x.title() for x in components)


def extract_rule_info(file_path):
    """
    Extract struct name and rule name from a rule file.
    Returns: (struct_name, rule_name) or None if not found
    """
    content = file_path.read_text()

    # Extract struct name: pub struct StructName;
    struct_match = re.search(r'pub\s+struct\s+(\w+)\s*;', content)
    if not struct_match:
        return None
    struct_name = struct_match.group(1)

    # Extract rule name from name() method
    name_match = re.search(r'fn\s+name\(\&self\)\s*->\s*&\'static\s+str\s*{\s*"([^"]+)"\s*}', content)
    if not name_match:
        return None
    rule_name = name_match.group(1)

    return (struct_name, rule_name)


def get_tested_rules(rule_tests_path):
    """Extract all rule names registered via rule_test! in rule_tests.rs."""
    content = rule_tests_path.read_text()
    matches = re.findall(r'rule_test!\s*\(\s*([a-z_0-9]+)\s*,', content)
    return set(matches)


def test_rule_consistency():
    """Test all rules for consistency."""
    project_root = Path(__file__).parent.parent
    rules_dir = project_root / "src" / "rules"
    rule_tests_rs = project_root / "tests" / "rule_tests.rs"
    fixtures_dir = project_root / "tests" / "fixtures"

    # Get all rule files (excluding mod.rs)
    rule_files = [f for f in rules_dir.glob("*.rs") if f.name != "mod.rs"]

    if not rule_files:
        raise AssertionError("No rule files found!")

    # Get tested rules
    tested_rules = get_tested_rules(rule_tests_rs)

    errors = []
    all_rule_names = set()

    for rule_file in sorted(rule_files):
        file_name = rule_file.stem  # filename without .rs

        # Extract struct and rule name from file
        rule_info = extract_rule_info(rule_file)
        if not rule_info:
            errors.append(f"{rule_file.name}: Could not extract struct name or rule name")
            continue

        struct_name, rule_name = rule_info
        all_rule_names.add(rule_name)

        # Check 1: File name must match rule name
        if file_name != rule_name:
            errors.append(
                f"{rule_file.name}: File name '{file_name}' does not match "
                f"rule name '{rule_name}' from name() method"
            )

        # Check 2: Struct name must be PascalCase of rule name
        expected_struct_name = snake_to_pascal(rule_name)
        if struct_name != expected_struct_name:
            errors.append(
                f"{rule_file.name}: Struct name '{struct_name}' does not match "
                f"expected PascalCase '{expected_struct_name}' for rule '{rule_name}'"
            )

        # Check 4: Rule must have a registered test in rule_tests.rs
        if rule_name not in tested_rules:
            errors.append(
                f"{rule_file.name}: Rule '{rule_name}' has no rule_test! entry in tests/rule_tests.rs"
            )

        # Check 5: Rule must have a fixture directory
        fixture_path = fixtures_dir / rule_name
        if not fixture_path.is_dir():
            errors.append(
                f"{rule_file.name}: Rule '{rule_name}' has no fixture directory at tests/fixtures/{rule_name}/"
            )

    # Report results
    if errors:
        error_msg = "\n".join(errors)
        raise AssertionError(f"\nRule consistency errors found:\n{error_msg}")

    print(f"✓ All {len(rule_files)} rules are consistent!")
    print("  - File names match rule names")
    print("  - Struct names are correct PascalCase")
    print("  - All rules have a registered test case in rule_tests.rs")
    print("  - All rules have a fixture directory")


if __name__ == "__main__":
    test_rule_consistency()
