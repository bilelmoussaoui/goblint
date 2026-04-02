mod config;
mod reporter;
mod rules;
mod scanner;

use anyhow::Result;
use clap::Parser;
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

    if args.verbose {
        println!("Loaded configuration from: {}", args.config.display());
        println!("Scanning directory: {}", args.directory.display());
        if !config.ignore.is_empty() {
            println!("Ignore patterns: {:?}", config.ignore);
        }
    }

    // Scan files and run rules
    let violations = scanner::scan_directory(&args.directory, &config, args.verbose)?;

    // Report violations
    reporter::report_violations(&violations, args.verbose, &config);

    // Exit with error code if violations found
    if !violations.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}
