use std::{collections::HashMap, env, path::Path};

use colored::*;

use crate::{config::Config, rules::Violation};

pub fn report_violations(violations: &[Violation], verbose: bool, config: &Config) {
    // Check if we're outputting to a terminal
    use std::io::IsTerminal;
    let use_hyperlinks = std::io::stdout().is_terminal();

    if violations.is_empty() {
        if verbose {
            println!("{}", "No violations found!".green().bold());
        }
        return;
    }

    println!(
        "{}",
        format!("Found {} violation(s):", violations.len())
            .red()
            .bold()
    );
    println!();

    for violation in violations {
        // Create clickable link (or plain text if not a terminal)
        let file_link = create_clickable_link(
            &violation.file,
            violation.line,
            violation.column,
            &config.editor_url,
            use_hyperlinks,
        );

        println!("{}", file_link);

        // Show code snippet if available
        if let Some(ref snippet) = violation.snippet {
            // Add indentation to each line
            for line in snippet.lines() {
                println!("  {}", line.dimmed());
            }
        }

        let level_label = match violation.level {
            crate::config::RuleLevel::Error => "error:".red().bold(),
            crate::config::RuleLevel::Warn => "warning:".yellow().bold(),
            crate::config::RuleLevel::Ignore => {
                unreachable!("Ignored violations should not be reported")
            }
        };
        println!("  {} {}", level_label, violation.message);
        println!("  {} {}", "rule:".blue(), violation.rule);
        println!();
    }
}

/// Print a summary table of violation counts grouped by rule, sorted by count
/// descending. `fixable` maps rule name → whether the rule supports auto-fix.
pub fn report_summary(violations: &[Violation], fixable: &HashMap<&str, bool>) {
    if violations.is_empty() {
        println!("{}", "No violations found!".green().bold());
        return;
    }

    // Aggregate counts per rule.
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for v in violations {
        *counts.entry(v.rule).or_insert(0) += 1;
    }

    // Build sorted rows: (rule, count, fixable), descending by count.
    let mut rows: Vec<(&str, usize, bool)> = counts
        .iter()
        .map(|(&rule, &count)| (rule, count, *fixable.get(rule).unwrap_or(&false)))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));

    // Column widths — at least wide enough for the header labels.
    let count_w = rows
        .iter()
        .map(|(_, c, _)| c.to_string().len())
        .max()
        .unwrap_or(0)
        .max("Count".len());
    let rule_w = rows
        .iter()
        .map(|(r, ..)| r.len())
        .max()
        .unwrap_or(0)
        .max("Rule".len());
    let fix_w = "Autofix".len(); // "Yes" / "No " padded to this width

    // Helper closures for border rows.
    let top = format!(
        "┌{:─<cw$}┬{:─<rw$}┬{:─<fw$}┐",
        "",
        "",
        "",
        cw = count_w + 2,
        rw = rule_w + 2,
        fw = fix_w + 2,
    );
    let sep = format!(
        "├{:─<cw$}┼{:─<rw$}┼{:─<fw$}┤",
        "",
        "",
        "",
        cw = count_w + 2,
        rw = rule_w + 2,
        fw = fix_w + 2,
    );
    let bot = format!(
        "└{:─<cw$}┴{:─<rw$}┴{:─<fw$}┘",
        "",
        "",
        "",
        cw = count_w + 2,
        rw = rule_w + 2,
        fw = fix_w + 2,
    );

    println!("{}", top);
    println!(
        "│ {:<cw$} │ {:<rw$} │ {:<fw$} │",
        "Count".bold(),
        "Rule".bold(),
        "Autofix".bold(),
        cw = count_w,
        rw = rule_w,
        fw = fix_w,
    );
    println!("{}", sep);

    for (rule, count, is_fixable) in &rows {
        let count_str = count.to_string().yellow().to_string();
        let rule_str = rule.cyan().to_string();
        let fix_str = if *is_fixable {
            "Yes".green().to_string()
        } else {
            "No".dimmed().to_string()
        };

        // ANSI escape codes inflate the byte length of colored strings, so we
        // pad the *visible* widths by computing the difference and adding it.
        let count_pad = count_w - count.to_string().len();
        let rule_pad = rule_w - rule.len();
        let fix_pad = fix_w - if *is_fixable { 3 } else { 2 };

        println!(
            "│ {}{} │ {}{} │ {}{} │",
            count_str,
            " ".repeat(count_pad),
            rule_str,
            " ".repeat(rule_pad),
            fix_str,
            " ".repeat(fix_pad),
        );
    }

    println!("{}", bot);
    println!(
        "  {} violation(s) across {} rule(s)",
        violations.len().to_string().yellow().bold(),
        rows.len().to_string().yellow().bold(),
    );
}

/// Report violations in GCC-compatible format for Emacs, Vim, and other tools
/// Format: path/to/file.c:line:column: level: message [rule_name]
pub fn report_violations_gcc(violations: &[Violation], project_root: &Path) {
    for violation in violations {
        // Make path relative to project root for cleaner output
        let relative_path = violation
            .file
            .strip_prefix(project_root)
            .unwrap_or(&violation.file);

        let level = match violation.level {
            crate::config::RuleLevel::Error => "error",
            crate::config::RuleLevel::Warn => "warning",
            crate::config::RuleLevel::Ignore => {
                unreachable!("Ignored violations should not be reported")
            }
        };

        println!(
            "{}:{}:{}: {}: {} [{}]",
            relative_path.display(),
            violation.line,
            violation.column,
            level,
            violation.message,
            violation.rule
        );
    }
}

fn create_clickable_link(
    file_path: &std::path::Path,
    line: usize,
    column: usize,
    editor_url_template: &Option<String>,
    use_hyperlinks: bool,
) -> String {
    // Convert to absolute path if relative
    let abs_path = if file_path.is_absolute() {
        file_path
    } else {
        match env::current_dir() {
            Ok(cwd) => &cwd.join(file_path),
            Err(_) => file_path,
        }
    };

    // Format: file:line:column
    let location = format!("{}:{}:{}", abs_path.display(), line, column);

    if !use_hyperlinks {
        // Plain text output for pipes, redirects, etc. - no colors, no hyperlinks
        return location;
    }

    // Use configured editor URL or default
    let file_url = if let Some(template) = editor_url_template {
        template
            .replace("{path}", &abs_path.display().to_string())
            .replace("{line}", &line.to_string())
            .replace("{column}", &column.to_string())
    } else {
        // Default: just use file:// protocol
        format!("file://{}", abs_path.display())
    };

    // OSC 8 hyperlink escape sequence with colored location
    let hyperlink = format!(
        "\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\",
        file_url,
        location.cyan()
    );

    hyperlink
}
