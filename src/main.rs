mod ast_context;
mod config;
mod reporter;
mod rules;
mod scanner;

use anyhow::Result;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "gobject-lint")]
#[command(about = "A fast linter for GObject/C code", long_about = None)]
struct Args {
    /// Directory to scan for C files
    #[arg(value_name = "DIRECTORY", default_value = ".")]
    directory: PathBuf,

    /// Path to configuration file
    #[arg(short, long, value_name = "FILE", default_value = "gobject-lint.toml")]
    config: PathBuf,

    /// Additional ignore patterns (can be specified multiple times)
    #[arg(short, long, value_name = "PATTERN")]
    ignore: Vec<String>,

    /// Show verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Load configuration
    let mut config = config::Config::load(&args.config)?;

    // Merge CLI ignore patterns with config
    config.ignore.extend(args.ignore.clone());

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
        &args.directory,
        &ignore_matcher,
        spinner.as_ref(),
    )?;

    // Run AST-based rules
    let violations =
        scanner::scan_with_ast(&ast_context, &config, &args.directory, spinner.as_ref())?;

    if let Some(sp) = spinner {
        sp.finish_and_clear();
    }

    if args.verbose {
        let total_functions: usize = ast_context
            .project
            .files
            .values()
            .map(|f| f.functions.len())
            .sum();
        let total_gobject_types: usize = ast_context
            .project
            .files
            .values()
            .map(|f| f.gobject_types.len())
            .sum();
        println!(
            "Parsed {} files, {} functions, {} GObject types",
            ast_context.project.files.len(),
            total_functions,
            total_gobject_types
        );
    }

    // Report violations
    reporter::report_violations(&violations, args.verbose, &config);

    // Exit with error code if violations found
    if !violations.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}
