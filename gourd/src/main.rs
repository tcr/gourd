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
use std::process::Command;

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
    /// Transpile Go code, build in a temp Cargo project, and run it
    Run {
        /// Input source: file path, inline Go code, or `-` for stdin
        input: String,
        #[arg(short, long, help = "Pass additional cargo args (e.g., --features)")]
        cargo_args: Vec<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Transpile { input } => {
            run_transpile(input);
        }
        Commands::Run { input, cargo_args } => {
            run_and_execute(&input, &cargo_args);
        }
    }
}

fn run_transpile(input: String) {
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

/// Transpile Go code, build in a temp Cargo project, and run it.
fn run_and_execute(input: &str, cargo_args: &[String]) {
    let source = read_source(input);

    // Transpile Go to Rust
    let rust = transpile_go_text(&source);
    let rust_code = format_rust_output(&rust);

    // Create temp Cargo project
    let temp_dir = tempfile::tempdir().unwrap_or_else(|e| {
        eprintln!("failed to create temp dir: {}", e);
        std::process::exit(1);
    });

    // Write Cargo.toml
    let manifest_path = temp_dir.path().join("Cargo.toml");
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap();
    std::fs::write(
        &manifest_path,
        format!(
            r#"[package]
name = "gourd-run"
version = "0.0.0"
edition = "2024"

[dependencies]
gourd = {{ path = "{}" }}
crossbeam = "0.8"
num-traits = "0.2"
"#,
            workspace_root.join("gourd").display(),
        ),
    )
    .unwrap_or_else(|e| {
        eprintln!("failed to write Cargo.toml: {}", e);
        std::process::exit(1);
    });

    // Create src/ directory and write transpiled code
    let src_dir = temp_dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap_or_else(|e| {
        eprintln!("failed to create src/: {}", e);
        std::process::exit(1);
    });

    let main_path = src_dir.join("main.rs");
    // Wrap in a minimal main if the transpiled code doesn't have one
    let has_main = rust_code.contains("fn main");
    let has_prelude = rust_code.contains("gourd::prelude");
    let code = if has_main {
        rust_code
    } else {
        format!("fn main() {{}}\n{}", rust_code)
    };
    // Prepend prelude import if the code uses gourd prelude functions
    let code = if has_prelude {
        format!("use gourd::prelude::*;\n{}", code)
    } else {
        code
    };
    std::fs::write(&main_path, &code).unwrap_or_else(|e| {
        eprintln!("failed to write main.rs: {}", e);
        std::process::exit(1);
    });

    // Run `cargo run --manifest-path` with any additional args
    let mut cmd = Command::new("cargo");
    cmd.arg("run")
       .arg("--manifest-path")
       .arg(&manifest_path)
       .args(cargo_args);

    let status = cmd.status().unwrap_or_else(|e| {
        eprintln!("failed to run cargo: {}", e);
        std::process::exit(1);
    });

    if !status.success() {
        std::process::exit(1);
    }
}

/// Parse transpiled TokenStream into a syn::File and format with prettyplease.
fn format_rust_output(ts: &TokenStream) -> String {
    if let Ok(file) = syn::parse2::<syn::File>(ts.clone()) {
        prettyplease::unparse(&file).trim().to_string()
    } else {
        ts.to_string().trim().to_string()
    }
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
