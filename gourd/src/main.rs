//! `gourd` CLI: Go → Rust transpiler.
//!
//! Transpile Go code (in `go!` macro blocks or raw `.go` files) to equivalent Rust.
//!
//! ## Usage
//!
//! ```bash
//! # Transpile Go blocks from a Rust source file
//! gourd transpile path/to/file.rs
//!
//! # Transpile inline Go code
//! gourd transpile "func hello() int { return 42 }"
//!
//! # Transpile from stdin
//! echo "func hello() int { return 42 }" | gourd transpile -
//! ```

use clap::{Parser, Subcommand};
use gourd::scanner::{find_go_blocks_from_source, scan_path};
use gourd::transpile_go_text;
use proc_macro2::TokenStream;
use std::io::Read;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "gourd")]
#[command(about = "Transpile Go code to Rust", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Transpile Go code to Rust
    Transpile {
        /// Input source: file path, inline Go code, or `-` for stdin
        input: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Transpile { input } => {
            let source = read_source(&input);

            // Inline Go code: read_source already wraps in go! { ... }, so just find blocks
            if !input.ends_with(".go") && !input.ends_with(".rs") && input != "-" {
                let blocks = find_go_blocks_from_source(&source, &input);

                if blocks.is_empty() {
                    // Fallback: transpile the raw source directly
                    let rust = transpile_go_text(&input);
                    println!("{}", format_rust_output(&rust));
                    return;
                }

                for block in &blocks {
                    let rust = transpile_go_text(&block.content);
                    println!("{}", format_rust_output(&rust));
                }
                return;
            }

            // File-based: scan the file
            let path = PathBuf::from(&input);
            let blocks = scan_path(&path).unwrap_or_else(|e| {
                eprintln!("error scanning '{}': {}", input, e);
                std::process::exit(1);
            });

            if blocks.is_empty() {
                // No go! blocks — print the Go code as-is (or transpile inline)
                let rust = transpile_go_text(&source);
                println!("{}", format_rust_output(&rust));
                return;
            }

            for block in &blocks {
                let rust = transpile_go_text(&block.content);
                println!("{}", format_rust_output(&rust));
            }
        }
    }
}

/// Parse transpiled TokenStream into a syn::File and format with prettyplease.
fn format_rust_output(ts: &TokenStream) -> String {
    ts.to_string().trim().to_string()
}

/// Read source text from file, stdin, or treat input as inline Go code.
fn read_source(input: &str) -> String {
    if input == "-" {
        let mut source = String::new();
        std::io::stdin()
            .read_to_string(&mut source)
            .expect("failed to read stdin");
        source
    } else if input.ends_with(".go") || input.ends_with(".rs") {
        std::fs::read_to_string(input)
            .unwrap_or_else(|e| {
                eprintln!("error: cannot read file '{}': {}", input, e);
                std::process::exit(1);
            })
    } else {
        // Inline Go code — wrap in macro invocation so find_go_blocks can extract it
        let marker = String::from("go!");
        format!("{} {{ {} }}", marker, input)
    }
}
