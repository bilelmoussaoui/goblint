use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use goblint::{ast_context, config, fixer, output, reporter, rules::Category, scanner};
use indicatif::{ProgressBar, ProgressStyle};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    /// Human-readable colorized output (default)
    Text,
    /// SARIF JSON format for GitHub Code Scanning, VS Code, etc.
    Sarif,
    /// GCC-compatible format for Emacs, Vim, and other tools
    Gcc,
}

#[derive(Parser, Debug)]
#[command(name = "goblint")]
#[command(about = "A fast linter for GObject/C code", long_about = None)]
struct Args {
    /// Directory to scan for C files
    #[arg(value_name = "DIRECTORY", default_value = ".")]
    directory: PathBuf,

    /// Path to configuration file
    #[arg(short, long, value_name = "FILE", default_value = concat!(env!("CARGO_PKG_NAME"), ".toml"))]
    config: PathBuf,

    /// Additional ignore patterns (can be specified multiple times)
    #[arg(short, long, value_name = "PATTERN")]
    ignore: Vec<String>,

    /// Show verbose output
    #[arg(short, long)]
    verbose: bool,

    /// List all available lint rules
    #[arg(long)]
    list_rules: bool,

    /// Enable only specific rules (can be repeated, overrides config)
    #[arg(long, value_name = "RULE")]
    only: Vec<String>,

    /// Filter rules by category
    #[arg(long, value_name = "CATEGORY")]
    category: Option<Category>,

    /// Output format
    #[arg(long, value_enum, default_value = "text")]
    format: OutputFormat,

    /// Automatically apply fixes for violations
    #[arg(long)]
    fix: bool,

    /// Print a summary table of violation counts grouped by rule
    #[arg(long)]
    summary: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Auto-disable colors if not outputting to a terminal (for Emacs, pipes, etc.)
    // unless using gcc format (which never uses colors)
    if !matches!(args.format, OutputFormat::Gcc) {
        use std::io::IsTerminal;
        if !std::io::stdout().is_terminal() {
            colored::control::set_override(false);
        }
    } else {
        // GCC format never uses colors
        colored::control::set_override(false);
    }

    // Load configuration
    let mut config = config::Config::load(&args.config)?;

    // Merge CLI ignore patterns with config
    config.ignore.extend(args.ignore.clone());

    // Apply --only filter if specified
    if !args.only.is_empty() {
        config.enable_only_rules(&args.only)?;
    }

    // Apply --category filter if specified
    if let Some(category) = args.category {
        config.filter_by_category(category)?;
    }

    // Handle --list-rules
    if args.list_rules {
        scanner::list_all_rules(&config);
        return Ok(());
    }

    // Canonicalize directory path for consistent path handling
    let project_root = args
        .directory
        .canonicalize()
        .unwrap_or(args.directory.clone());

    // Build ignore matcher
    let ignore_matcher = config.build_ignore_matcher()?;

    // Create spinner for progress
    let spinner = if args.verbose {
        let sp = ProgressBar::new_spinner();
        sp.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        sp.enable_steady_tick(std::time::Duration::from_millis(100));
        Some(sp)
    } else {
        None
    };

    // Build AST-based context
    if let Some(ref sp) = spinner {
        sp.set_message("Parsing files...");
    }
    let ast_context = ast_context::AstContext::build_with_ignore(
        &project_root,
        &ignore_matcher,
        spinner.as_ref(),
    )?;

    // Run AST-based rules
    let violations =
        scanner::scan_with_ast(&ast_context, &config, &project_root, spinner.as_ref())?;

    if let Some(sp) = spinner {
        sp.finish_and_clear();
    }

    if args.verbose {
        let total_functions: usize = ast_context
            .project
            .files
            .values()
            .map(|f| f.iter_function_declarations().count() + f.iter_function_definitions().count())
            .sum();
        let total_gobject_types: usize = ast_context
            .project
            .files
            .values()
            .map(|f| f.iter_all_gobject_types().count())
            .sum();
        println!(
            "Parsed {} files, {} functions, {} GObject types",
            ast_context.project.files.len(),
            total_functions,
            total_gobject_types
        );
    }

    // Apply fixes if --fix was passed
    if args.fix {
        // Check if any enabled rules are fixable
        let rules = scanner::create_all_rules(&config);
        let has_fixable_rules = rules
            .iter()
            .any(|entry| entry.level.is_enabled() && entry.rule.fixable());

        if !has_fixable_rules {
            eprintln!(
                "Warning: --fix was specified but no enabled rules are auto-fixable.\n\
                 Run `goblin --list-rules` to see which rules support auto-fix."
            );
        } else {
            let fixed_count = fixer::apply_fixes(&violations)?;
            println!("Fixed {} violation(s)", fixed_count);
        }

        // Don't exit with error code when we fixed things
        return Ok(());
    }

    // Summary table mode
    if args.summary {
        let rules = scanner::create_all_rules(&config);
        let fixable: std::collections::HashMap<&str, bool> = rules
            .iter()
            .map(|e| (e.rule.name(), e.rule.fixable()))
            .collect();
        reporter::report_summary(&violations, &fixable);
        let has_errors = violations.iter().any(|v| v.level.is_error());
        if has_errors {
            std::process::exit(1);
        }
        return Ok(());
    }

    // Output violations in the requested format
    match args.format {
        OutputFormat::Text => {
            reporter::report_violations(&violations, args.verbose, &config);
        }
        OutputFormat::Sarif => {
            let sarif_output = output::sarif::generate_sarif(&violations, &config, &project_root);
            println!("{}", sarif_output);
        }
        OutputFormat::Gcc => {
            reporter::report_violations_gcc(&violations, &project_root);
        }
    }

    // Exit with error code only if there are error-level violations (not warnings)
    let has_errors = violations.iter().any(|v| v.level.is_error());
    if has_errors {
        std::process::exit(1);
    }

    Ok(())
}
