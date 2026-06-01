//! Semantic validation of Go and Rust code using real compilers.

use crate::scanner::{GoBlock, VerifyBlock};
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

/// Validate Go blocks by running `go build` on each.
pub fn validate_go(blocks: &[GoBlock]) -> Vec<CheckResult> {
    blocks
        .iter()
        .map(|block| {
            let code = block.content.clone();
            // Each block gets its own temp dir to avoid file conflicts.
            let tmp = tempfile::tempdir().unwrap();
            std::fs::write(tmp.path().join("go.mod"), "module gourd-test\ngo 1.21\n").ok();
            let go_result = run_go_build(&tmp, &code);
            CheckResult {
                file: block.file.clone(),
                line: block.line,
                go_code: code,
                go_valid: match go_result {
                    Ok(()) => Some(Validation::Ok),
                    Err(e) => Some(Validation::Error(e.to_string())),
                },
                rust_valid: None,
            }
        })
        .collect()
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
