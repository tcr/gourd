//! `gourd-check`: standalone CLI for semantic validation of Go blocks.
//!
//! Scans Rust source files for `go!` blocks, extracts the exact
//! source text, and validates using `go build` and `cargo check`.

mod scanner;
mod validator;
mod report;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "gourd-check")]
#[command(version, about = "Validate Go blocks in gourd source files")]
struct Cli {
    /// Path(s) to scan (default: current directory)
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,

    /// Only validate Go code (default: both Go and Rust)
    #[arg(short, long)]
    go_only: bool,

    /// Only validate transpiled Rust code
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
        // Print full details for debugging
        for r in &results {
            eprintln!("Block: {}:{} ({} bytes)", r.file, r.line, r.go_code.len());
        }
    }
    println!("{}", report::format_results(&results));
    if results.iter().any(|r| {
        r.go_valid.as_ref().map_or(false, |v| matches!(v, validator::Validation::Error(_)))
            || r.rust_valid.as_ref().map_or(false, |v| matches!(v, validator::Validation::Error(_)))
    }) {
        std::process::exit(1);
    }
    Ok(())
}

fn run_check(cli: &Cli) -> Result<Vec<validator::CheckResult>> {
    let mut all_blocks = Vec::new();

    for path in &cli.paths {
        all_blocks.extend(scanner::scan_path(path)?);
    }

    let results: Vec<validator::CheckResult> = if cli.go_only {
        validator::validate_go(&all_blocks)
    } else {
        validator::validate_go(&all_blocks)
    };

    if cli.verbose >= 2 {
        eprintln!("Found {} blocks, validated {}", all_blocks.len(), results.len());
    }

    Ok(results)
}
