//! Rust validation.
//!
//! Validates Rust code by running `cargo check` on a temporary directory.

use std::fs;
use std::process::Command;
use proc_macro2::TokenStream;

use super::temp::temp_dir;
use super::helpers::rust_to_main_harness;

/// Validate that `rust_code` would compile as valid Rust.
///
/// Writes the code to a temporary Rust project directory and runs
/// `cargo check`. Returns `Err(msg)` with the compiler output if it fails.
///
/// The input should be Rust **declarations** (structs, functions, impls, etc.).
/// A minimal main function is added automatically.
pub fn validate_rust(rust_code: &TokenStream) -> Result<(), String> {
    let dir = temp_dir("gourd-rust");
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create temp dir: {}", e))?;
    fs::write(dir.join("Cargo.toml"),
        "[package]\nname = \"gourd-test\"\nversion = \"0.0.0\"\nedition = \"2021\"\n")
        .map_err(|e| format!("failed to write Cargo.toml: {}", e))?;
    let src = dir.join("src");
    fs::create_dir_all(&src).map_err(|e| format!("failed to create src/: {}", e))?;
    fs::write(src.join("main.rs"), rust_to_main_harness(rust_code))
        .map_err(|e| format!("failed to write src/main.rs: {}", e))?;

    let output = Command::new("cargo")
        .args(["check", "-q"])
        .current_dir(&dir)
        .output()
        .map_err(|e| format!("failed to run `cargo check`: {}", e))?;

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        fs::remove_dir_all(&dir).ok();
        return Err(stderr.trim().to_string());
    }

    fs::remove_dir_all(&dir).ok();
    Ok(())
}
