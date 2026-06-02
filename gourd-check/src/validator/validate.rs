//! Validation functions.
//!
//! Validates Go blocks by running `go build` and Rust blocks by
//! running `cargo check`.

use std::collections::BTreeMap;

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
        "[package]\nname = \"gourd-test\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .ok();

    // Wrap in a minimal Rust file so cargo check can run it
    let wrapped = format!("fn main() {{}}\n\n{}\n", code);

    let main_rs = src.join("main.rs");
    std::fs::write(&main_rs, &wrapped).ok();

    match run_cargo_check(tmp.path(), &wrapped) {
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
