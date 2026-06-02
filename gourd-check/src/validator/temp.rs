//! Temporary directory functions.
//!
//! Provides functions for running `go build` and `cargo check` on
//! temporary directories.

use std::process::Command;
use std::io;
use std::path::Path;

/// Run `go build` on a temporary directory.
pub fn run_go_build(dir: &Path, code: &str) -> io::Result<()> {
    let main_go = dir.join("main.go");
    // Wrap in a minimal Go file so the compiler can parse it.
    let wrapped = format!(
        "package main\n\nimport (\n    \"fmt\"\n    \"unsafe\"\n)\n\nvar _ = fmt.Print\nvar _ = unsafe.Sizeof(0)\n\nfunc main() {{}}\n\n{code}\n",
    );
    std::fs::write(&main_go, &wrapped)?;

    let output = Command::new("go")
        .args(["build", "-o", "/dev/null"])
        .current_dir(dir)
        .output()?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(io::Error::new(io::ErrorKind::Other, stderr.trim().to_string()))
    }
}

/// Run `cargo check` on a temporary directory.
pub fn run_cargo_check(dir: &Path, code: &str) -> io::Result<()> {
    let main_rs = dir.join("src").join("main.rs");
    std::fs::write(&main_rs, code)?;

    let output = Command::new("cargo")
        .args(["check", "-q"])
        .current_dir(dir)
        .output()?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(io::Error::new(io::ErrorKind::Other, stderr.trim().to_string()))
    }
}
