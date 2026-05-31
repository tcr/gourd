//! Semantic validation of Go and Rust code using real compilers.

use crate::scanner::GoBlock;
use std::process::Command;
use tempfile::TempDir;

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

/// Validate Rust blocks by running `cargo check` on each.
#[allow(dead_code)]
pub fn validate_rust(blocks: &[GoBlock]) -> Vec<CheckResult> {
    blocks
        .iter()
        .map(|block| {
            let code = block.content.clone();
            let tmp = tempfile::tempdir().unwrap();
            std::fs::write(tmp.path().join("go.mod"), "module gourd-test\ngo 1.21\n").ok();
            let src = tmp.path().join("src");
            std::fs::create_dir_all(&src).ok();
            std::fs::write(
                tmp.path().join("Cargo.toml"),
                "[package]\nname = \"gourd-test\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
            )
            .ok();
            let rust_result = run_cargo_check(&tmp, &code);
            CheckResult {
                file: block.file.clone(),
                line: block.line,
                go_code: code,
                go_valid: None,
                rust_valid: match rust_result {
                    Ok(()) => Some(Validation::Ok),
                    Err(e) => Some(Validation::Error(e.to_string())),
                },
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

#[allow(dead_code)]
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
