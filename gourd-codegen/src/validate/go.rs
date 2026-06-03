//! Go validation.
//!
//! Validates Go code by running `go build` on a temporary directory.

use std::fs;
use std::process::Command;
use proc_macro2::TokenStream;

use super::temp::temp_dir;
use super::helpers::go_to_main_harness;

/// Validate that `go_code` would compile as valid Go.
///
/// Writes the code to a temporary Go project directory and runs
/// `go build`. Returns `Err(msg)` with the compiler output if it fails.
///
/// The input should be Go **declarations** (functions, structs, etc.).
/// A minimal `package main` with a `main()` function is added automatically.
pub fn validate_go(go_code: &TokenStream) -> Result<(), String> {
    let dir = temp_dir("gourd-go");
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create temp dir: {}", e))?;
    fs::write(dir.join("go.mod"), "module gourd-test\ngo 1.21\n")
        .map_err(|e| format!("failed to write go.mod: {}", e))?;
    let harness = go_to_main_harness(go_code);

    fs::write(dir.join("main.go"), harness)
        .map_err(|e| format!("failed to write main.go: {}", e))?;

    let output = Command::new("go")
        .args(["build", "-o", "/dev/null", "."])
        .current_dir(&dir)
        .output()
        .map_err(|e| format!("failed to run `go build`: {}", e))?;

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        fs::remove_dir_all(&dir).ok();
        return Err(stderr.trim().to_string());
    }

    fs::remove_dir_all(&dir).ok();
    Ok(())
}
