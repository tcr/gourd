//! Integration test for `gourd-check`.
//!
//! Scans the entire workspace for `go!` blocks and `#[verify_rust_output]`
//! attributes, then validates both using the real compilers (`go build` and
//! `cargo check`), matching the behavior of `gourd-check main`.
//!
//! Split into two tests:
//! - `test_go_validation` — parallelized across file groups with rayon
//! - `test_verify_validation` — batched: all verify blocks combined into
//!   a single source file and validated with ONE `cargo check`

use std::path::PathBuf;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// ── Go validation (parallel across file groups) ──────────────────────────
#[test]
fn test_go_validation() {
    let workspace = workspace_root();
    let go_blocks = gourd_check::scanner::scan_path(&workspace).expect("scan workspace");

    // Group by file, then validate each group in parallel
    let mut groups: std::collections::BTreeMap<String, Vec<&gourd_check::scanner::GoBlock>> =
        std::collections::BTreeMap::new();
    for block in &go_blocks {
        groups.entry(block.file.clone()).or_default().push(block);
    }

    // Validate each file group in parallel
    let slices: Vec<_> = groups.values().map(|v| v.as_slice()).collect();
    let results: Vec<_> = slices.par_iter()
        .map(|group| gourd_check::validator::validate_go_file_group(group))
        .collect();
    let go_results: Vec<_> = results.into_iter().flatten().collect();

    let go_errors: Vec<_> = go_results
        .iter()
        .filter_map(|r| {
            r.go_valid.as_ref().and_then(|v| match v {
                gourd_check::validator::Validation::Error(e) => {
                    Some(format!("Go: {}:{}: {}", r.file, r.line, e))
                }
                _ => None,
            })
        })
        .collect();

    if !go_errors.is_empty() {
        eprintln!("Go validation errors:\n{}", go_errors.join("\n"));
    }
    assert!(
        go_errors.is_empty(),
        "{} Go block(s) failed validation",
        go_errors.len()
    );
}

/// ── Verify block validation (batched — one cargo check) ──────────────────
#[test]
fn test_verify_validation() {
    let workspace = workspace_root();
    let verify_blocks = gourd_check::scanner::scan_verify(&workspace).expect("scan verify");

    // Batch all verify blocks into one cargo check
    let verify_results = gourd_check::validator::validate_verify_blocks_batched(&verify_blocks);

    let rust_errors: Vec<_> = verify_results
        .iter()
        .filter_map(|r| {
            match &r.validation {
                Some(gourd_check::validator::Validation::Error(e)) => {
                    Some(format!("Rust: {}:{}: {}", r.file, r.line, e))
                }
                _ => None,
            }
        })
        .collect();

    if !rust_errors.is_empty() {
        eprintln!("Rust verification errors:\n{}", rust_errors.join("\n"));
    }
    assert!(
        rust_errors.is_empty(),
        "{} verify block(s) failed validation",
        rust_errors.len()
    );
}
