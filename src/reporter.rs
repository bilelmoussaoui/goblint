use std::env;

use colored::*;

use crate::{config::Config, rules::Violation};

pub fn report_violations(violations: &[Violation], verbose: bool, config: &Config) {
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
        // Create clickable link
        let file_link = create_clickable_link(
            &violation.file,
            violation.line,
            violation.column,
            &config.editor_url,
        );

        println!("{}", file_link);

        // Show code snippet if available
        if let Some(ref snippet) = violation.snippet {
            println!("  {}", snippet.dimmed());
        }

        println!("  {} {}", "error:".red().bold(), violation.message);
        println!("  {} {}", "rule:".blue(), violation.rule);
        println!();
    }
}

fn create_clickable_link(
    file_path: &std::path::Path,
    line: usize,
    column: usize,
    editor_url_template: &Option<String>,
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

    // OSC 8 hyperlink escape sequence
    let hyperlink = format!(
        "\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\",
        file_url,
        location.cyan()
    );

    hyperlink
}
