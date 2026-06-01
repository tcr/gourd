//! `gourd-check`: standalone CLI for semantic validation of Go blocks.
//!
//! Scans Rust source files for `go!` blocks, extracts the exact
//! source text, and validates using `go build` and `cargo check`.
//!
//! Also validates `#[verify_rust_output]` attributes by extracting
//! the expected Rust tokens and running `cargo check` on them.

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

// Re-export lib crate modules for CLI use
use gourd_check::{scanner, validator, report};

#[derive(Parser, Debug)]
#[command(name = "gourd-check")]
#[command(version, about = "Validate Go blocks in gourd source files")]
struct Cli {
    /// Path(s) to scan (default: current directory)
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,

    /// Only validate Go code
    #[arg(short, long)]
    go_only: bool,

    /// Only validate verify_rust_output Rust code
    #[arg(short, long)]
    rust_only: bool,

    /// Verbosity: 0=quiet, 1=summary, 2=full details
    #[arg(short, long, default_value = "1")]
    verbose: u8,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let results = run_check(&cli)?;
    if cli.verbose >= 2 {
        for r in &results {
            eprintln!("Block: {}:{} ({} bytes)", r.file, r.line, r.content.len());
        }
    }
    println!("{}", report::format_results(&results));
    if results.iter().any(|r| {
        matches!(r.validation.as_ref(), Some(validator::Validation::Error(_)))
    }) {
        std::process::exit(1);
    }
    Ok(())
}

fn run_check(cli: &Cli) -> Result<Vec<validator::FormatResult>> {
    let mut all_results: Vec<validator::FormatResult> = Vec::new();

    for path in &cli.paths {
        if cli.go_only {
            let blocks = scanner::scan_path(path)?;
            let results = validator::validate_go(&blocks);
            all_results.extend(validator::check_results_to_format(results));
        } else if cli.rust_only {
            let blocks = scanner::scan_verify(path)?;
            let results = validator::validate_verify_blocks(&blocks);
            all_results.extend(validator::verify_checks_to_format(results));
        } else {
            // Default: validate both Go blocks and verify attributes
            let go_blocks = scanner::scan_path(path)?;
            let go_results = validator::validate_go(&go_blocks);
            all_results.extend(validator::check_results_to_format(go_results));

            let verify_blocks = scanner::scan_verify(path)?;
            let verify_results = validator::validate_verify_blocks(&verify_blocks);
            all_results.extend(validator::verify_checks_to_format(verify_results));
        }
    }

    Ok(all_results)
}
