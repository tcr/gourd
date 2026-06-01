//! Semantic validation of Go and Rust code using real compilers.

use crate::scanner::{GoBlock, VerifyBlock};
use std::collections::BTreeMap;
use std::process::Command;
use tempfile::TempDir;

/// A unified format result for both Go and Rust validation.
#[derive(Debug, Clone)]
pub struct FormatResult {
    pub file: String,
    pub line: usize,
    pub content: String,
    pub validation: Option<Validation>,
}

/// Result of validating a single Go block.
#[derive(Debug)]
pub struct CheckResult {
    pub file: String,
    pub line: usize,
    pub go_code: String,
    pub go_valid: Option<Validation>,
    pub rust_valid: Option<Validation>,
}

/// Whether a validation pass succeeded, failed, or was skipped.
#[derive(Debug, Clone)]
pub enum Validation {
    Ok,
    Error(String),
}

/// A discovered verify block ready for Rust validation.
#[derive(Debug, Clone)]
pub struct VerifyCheck {
    pub file: String,
    pub line: usize,
    pub rust_code: String,
    pub validation: Option<Validation>,
}

/// Convert CheckResults to FormatResults.
pub fn check_results_to_format(results: Vec<CheckResult>) -> Vec<FormatResult> {
    results
        .into_iter()
        .map(|r| FormatResult {
            file: r.file,
            line: r.line,
            content: r.go_code,
            validation: r.go_valid,
        })
        .collect()
}

/// Convert VerifyChecks to FormatResults.
pub fn verify_checks_to_format(results: Vec<VerifyCheck>) -> Vec<FormatResult> {
    results
        .into_iter()
        .map(|r| FormatResult {
            file: r.file,
            line: r.line,
            content: r.rust_code,
            validation: r.validation,
        })
        .collect()
}

/// Normalize Go dialect code for validation by the real Go compiler.
/// Converts:
/// - `while` loops → `for` loops (Go doesn't have `while`)
/// - `struct Name { ... }` → `type Name struct { ... }`
pub fn normalize_go_code(code: &str) -> String {
    // Step 1: `while` → `for`
    let step1 = normalize_while(code);
    // Step 2: `struct` → `type Name struct`
    normalize_struct(&step1)
}

/// Replace `while` with `for` at word boundaries.
fn normalize_while(code: &str) -> String {
    let mut result = String::with_capacity(code.len());
    let mut pos = 0;

    while pos < code.len() {
        if code[pos..].starts_with("while") {
            let after_end = pos + 5;
            let prev_is_ident = pos > 0 && {
                let prev: char = code.chars().nth(pos - 1).unwrap_or(' ');
                prev.is_alphanumeric() || prev == '_'
            };
            let after_is_ident = after_end < code.len() && {
                let after: char = code.chars().nth(after_end).unwrap_or(' ');
                after.is_alphanumeric() || after == '_'
            };
            if !prev_is_ident && !after_is_ident {
                result.push_str("for");
                pos += 5;
                continue;
            }
        }
        result.push(code.chars().nth(pos).unwrap());
        pos += 1;
    }
    result
}

/// Replace `struct Name { ... }` → `type Name struct { ... }`.
fn normalize_struct(code: &str) -> String {
    let mut result = String::new();
    let mut in_block = false;
    let mut block_depth = 0;

    for line in code.split('\n') {
        let trimmed = line.trim_start();

        // Track if we're inside a struct block
        if !in_block {
            if trimmed.starts_with("struct ") || trimmed.starts_with("struct{") {
                // This is a struct declaration line
                let indent: String = line.chars().take(line.len() - trimmed.len()).collect();
                let without_struct = &trimmed[7..]; // skip "struct "
                let name_end = without_struct.find(|c: char| c == '{' || c == '(')
                    .unwrap_or(without_struct.len());
                let name = without_struct[..name_end].trim();
                let rest = &without_struct[name_end..]; // "{ ... }" or "(...) { ... }"

                result.push_str(&indent);
                result.push_str(&format!("type {} struct{}", name, rest));
                in_block = true;
                block_depth = rest.matches('{').count();
                continue;
            }
        } else {
            // Inside a struct block, track brace depth
            for ch in line.chars() {
                if ch == '{' { block_depth += 1; }
                if ch == '}' { block_depth -= 1; }
            }
            result.push_str(line);
            if block_depth == 0 {
                in_block = false;
            }
        }

        result.push('\n');
    }

    // Remove trailing newline
    result.trim_end().to_string()
}

/// Validate a single verify block by running `cargo check` on the extracted Rust code.
pub fn validate_verify_block(code: &str) -> Validation {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).ok();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"gourd-test\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .ok();

    // Wrap in a minimal Rust file so cargo check can run it
    let wrapped = format!("fn main() {{}}\n\n{}\n", code);

    let main_rs = src.join("main.rs");
    std::fs::write(&main_rs, &wrapped).ok();

    match run_cargo_check(&tmp, &wrapped) {
        Ok(()) => Validation::Ok,
        Err(e) => Validation::Error(e.to_string()),
    }
}

/// Validate all verify blocks by running `cargo check` on each.
pub fn validate_verify_blocks(blocks: &[VerifyBlock]) -> Vec<VerifyCheck> {
    blocks
        .iter()
        .map(|block| {
            let validation = validate_verify_block(&block.content);
            VerifyCheck {
                file: block.file.clone(),
                line: block.line,
                rust_code: block.content.clone(),
                validation: Some(validation),
            }
        })
        .collect()
}

/// Validate Go blocks by running `go build` on groups per file.
/// Blocks from the same source file are combined so struct definitions
/// in one block are available to dependent function blocks.
pub fn validate_go(blocks: &[GoBlock]) -> Vec<CheckResult> {
    // Group blocks by source file
    let mut file_groups: BTreeMap<String, Vec<&GoBlock>> = BTreeMap::new();
    for block in blocks {
        file_groups
            .entry(block.file.clone())
            .or_default()
            .push(block);
    }

    let mut results = Vec::new();

    for (_file, group_blocks) in file_groups {
        // Combine all blocks from the same file into one Go source
        let combined = group_blocks
            .iter()
            .map(|b| b.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        // Normalize Go dialect before validation
        let normalized = normalize_go_code(&combined);

        // Validate all blocks from this file together
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("go.mod"), "module gourd-test\ngo 1.21\n").ok();
        let go_result = run_go_build(&tmp, &normalized);

        // Assign the validation result to each individual block
        for block in group_blocks {
            results.push(CheckResult {
                file: block.file.clone(),
                line: block.line,
                go_code: block.content.clone(),
                go_valid: match &go_result {
                    Ok(()) => Some(Validation::Ok),
                    Err(e) => Some(Validation::Error(e.to_string())),
                },
                rust_valid: None,
            });
        }
    }

    results
}

fn run_go_build(dir: &TempDir, code: &str) -> std::io::Result<()> {
    let main_go = dir.path().join("main.go");
    // Wrap in a minimal Go file so the compiler can parse it.
    let wrapped = format!(
        "package main\n\nimport (\n    \"fmt\"\n    \"unsafe\"\n)\n\nvar _ = fmt.Print\nvar _ = unsafe.Sizeof(0)\n\nfunc main() {{}}\n\n{code}\n",
    );
    std::fs::write(&main_go, &wrapped)?;

    let output = Command::new("go")
        .args(["build", "-o", "/dev/null"])
        .current_dir(dir.path())
        .output()?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(std::io::Error::new(std::io::ErrorKind::Other, stderr.trim().to_string()))
    }
}

fn run_cargo_check(dir: &TempDir, code: &str) -> std::io::Result<()> {
    let main_rs = dir.path().join("src").join("main.rs");
    std::fs::write(&main_rs, code)?;

    let output = Command::new("cargo")
        .args(["check", "-q"])
        .current_dir(dir.path())
        .output()?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(std::io::Error::new(std::io::ErrorKind::Other, stderr.trim().to_string()))
    }
}
