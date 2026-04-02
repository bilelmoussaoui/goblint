use crate::config::Config;
use crate::rules::{get_all_rules, Violation};
use anyhow::{Context, Result};
use globset::GlobSet;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::path::Path;
use tree_sitter::Parser;
use walkdir::WalkDir;

pub fn scan_directory(
    directory: &Path,
    config: &Config,
    show_progress: bool,
) -> Result<Vec<Violation>> {
    let mut violations = Vec::new();
    let rules = get_all_rules();

    // Filter enabled rules
    let enabled_rules: Vec<_> = rules
        .into_iter()
        .filter(|rule| rule.is_enabled(config))
        .collect();

    // Build ignore matcher
    let ignore_matcher = config.build_ignore_matcher()?;

    // Collect all files first to get count
    let files: Vec<_> = WalkDir::new(directory)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "c" || ext == "h")
        })
        .filter(|e| !should_ignore(e.path(), directory, &ignore_matcher))
        .collect();

    let total_files = files.len();

    // Create progress bar
    let progress = if show_progress && total_files > 0 {
        let pb = ProgressBar::new(total_files as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );
        Some(pb)
    } else {
        None
    };

    // Scan each file
    for entry in files {
        let path = entry.path();

        if let Some(ref pb) = progress {
            pb.set_message(format!("{}", path.display()));
        }

        match scan_file(path, &enabled_rules) {
            Ok(mut file_violations) => violations.append(&mut file_violations),
            Err(e) => eprintln!("Warning: Failed to scan {}: {}", path.display(), e),
        }

        if let Some(ref pb) = progress {
            pb.inc(1);
        }
    }

    if let Some(pb) = progress {
        pb.finish_with_message("Scan complete");
    }

    Ok(violations)
}

fn should_ignore(path: &Path, base_dir: &Path, matcher: &GlobSet) -> bool {
    // Get relative path from base directory
    let relative_path = match path.strip_prefix(base_dir) {
        Ok(rel) => rel,
        Err(_) => path,
    };

    matcher.is_match(relative_path)
}

fn scan_file(path: &Path, rules: &[Box<dyn crate::rules::Rule>]) -> Result<Vec<Violation>> {
    let mut violations = Vec::new();

    let source_code =
        fs::read(path).with_context(|| format!("Failed to read file: {}", path.display()))?;

    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_c::LANGUAGE.into())
        .context("Failed to load C grammar")?;

    let tree = parser
        .parse(&source_code, None)
        .context("Failed to parse C code")?;

    let root_node = tree.root_node();

    // Traverse the tree and check each node against all rules
    traverse_tree(root_node, &source_code, path, rules, &mut violations);

    // Add code snippets to violations
    add_snippets_to_violations(&mut violations, &source_code);

    Ok(violations)
}

fn add_snippets_to_violations(violations: &mut [Violation], source_code: &[u8]) {
    let source_str = String::from_utf8_lossy(source_code);
    let lines: Vec<&str> = source_str.lines().collect();

    for violation in violations.iter_mut() {
        if violation.line > 0 && violation.line <= lines.len() {
            let line_content = lines[violation.line - 1].trim();
            violation.snippet = Some(line_content.to_string());
        }
    }
}

fn traverse_tree(
    node: tree_sitter::Node,
    source: &[u8],
    path: &Path,
    rules: &[Box<dyn crate::rules::Rule>],
    violations: &mut Vec<Violation>,
) {
    // Check current node against all rules
    for rule in rules {
        violations.extend(rule.check(node, source, path));
    }

    // Recursively traverse children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        traverse_tree(child, source, path, rules, violations);
    }
}
