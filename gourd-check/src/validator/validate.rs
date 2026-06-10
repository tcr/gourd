//! Validation functions.
//!
//! Validates Go blocks by running `go build` and Rust blocks by
//! running `cargo check`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::scanner::{GoBlock, VerifyBlock};
use super::types::{CheckResult, Validation, VerifyCheck};
use super::normalize::normalize_go_code;
use super::temp::run_go_build;
use super::temp::run_cargo_check;

/// Validate a single verify block by running `cargo check` on the extracted Rust code.
pub fn validate_verify_block(code: &str) -> Validation {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).ok();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"gourd-test\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[dependencies]\ngourd = { path = \"../../gourd\" }\n",
    )
    .ok();

    // Resolve the absolute path to the gourd crate.
    // CARGO_MANIFEST_DIR for gourd-check is gourd-check/.
    // The workspace root is its parent.
    let gourd_path = {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().to_string_lossy().to_string());
        let manifest_path = std::path::Path::new(&manifest_dir);
        // Go up one level from gourd-check/ to workspace root, then add gourd/
        manifest_path.parent()
            .map(|p| p.join("gourd"))
            .unwrap_or_else(|| manifest_path.join("gourd"))
    };

    // Wrap in a minimal Rust file so cargo check can run it
    // Include prelude imports so HashMap and other prelude types resolve
    let wrapped = format!("use gourd::prelude::*;\nfn main() {{}}\n\n{}\n", code);

    let main_rs = src.join("main.rs");
    std::fs::write(&main_rs, &wrapped).ok();

    // Write Cargo.toml with gourd dependency using absolute path
    let cargo_toml = format!(
        "[package]\nname = \"gourd-test\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[dependencies]\ngourd = {{ path = \"{}\" }}\n",
        gourd_path.display()
    );
    std::fs::write(tmp.path().join("Cargo.toml"), &cargo_toml).ok();

    match run_cargo_check(tmp.path(), &wrapped) {
        Ok(()) => Validation::Ok,
        Err(e) => Validation::Error(e.to_string()),
    }
}

/// Validate all verify blocks by running `cargo check` on each.
///
/// Sets a shared CARGO_TARGET_DIR across all blocks so gourd compiles
/// once and subsequent checks reuse the compiled artifacts. Without
/// this, each verify block would take ~4s (compiling gourd fresh).
/// With shared target dir, total is ~10s for all 15 blocks.
pub fn validate_verify_blocks(blocks: &[VerifyBlock]) -> Vec<VerifyCheck> {
    // Create a shared temp target directory. Both the verify blocks and
    // cargo will use this same directory, so gordo compiles once in the
    // first block and subsequent blocks reuse the cached build.
    let shared_target = tempfile::tempdir().ok();

    // Set CARGO_TARGET_DIR upfront so all verify blocks share the cache
    if let Some(ref target) = shared_target {
        unsafe { std::env::set_var("CARGO_TARGET_DIR", target.path()) };
    }

    let results: Vec<VerifyCheck> = blocks
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
        .collect();

    // Unset CARGO_TARGET_DIR to avoid polluting subsequent tests
    if shared_target.is_some() {
        unsafe { std::env::remove_var("CARGO_TARGET_DIR") };
    }

    results
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
        let go_result = run_go_build(tmp.path(), &normalized);

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
