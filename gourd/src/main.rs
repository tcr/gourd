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
use gourd_check::scanner::find_go_blocks;
use gourd_codegen_core::transpile_go_text;
use proc_macro2::TokenStream;
use std::io::Read;

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
            let blocks = find_go_blocks(&source, &input);

            if blocks.is_empty() {
                // No go! blocks found — treat entire source as inline Go code
                let rust = transpile_go_text(&source);
                eprintln!("{}", &input);
                println!("{}", format_rust_output(&rust));
                return;
            }

            for block in &blocks {
                let rust = transpile_go_text(&block.content);
                eprintln!("{}:{}", &input, block.line);
                println!("{}", format_rust_output(&rust));
            }
        }
    }
}

/// Parse transpiled TokenStream into a syn::File and format with prettyplease.
fn format_rust_output(ts: &TokenStream) -> String {
    let ast: syn::File = match syn::parse2(ts.clone()) {
        Ok(ast) => ast,
        Err(_) => return ts.to_string().trim().to_string(),
    };
    prettyplease::unparse(&ast)
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
