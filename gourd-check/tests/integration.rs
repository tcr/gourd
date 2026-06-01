//! Integration test for `gourd-check`.
//!
//! Scans the entire workspace for `go!` blocks and `#[verify_rust_output]`
//! attributes, then validates both using the real compilers (`go build` and
//! `cargo check`), matching the behavior of `gourd-check main`.

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

#[test]
fn test_gourd_check_validates_all_blocks() {
    let workspace = workspace_root();

    // Validate Go blocks
    let go_blocks = gourd_check::scanner::scan_path(&workspace).expect("scan workspace");
    let go_results = gourd_check::validator::validate_go(&go_blocks);

    // Validate verify_rust_output blocks
    let verify_blocks = gourd_check::scanner::scan_verify(&workspace).expect("scan verify");
    let verify_results = gourd_check::validator::validate_verify_blocks(&verify_blocks);

    // Go errors
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

    // Rust errors
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

    let all_errors: Vec<String> = go_errors.into_iter().chain(rust_errors.into_iter()).collect();

    if !all_errors.is_empty() {
        eprintln!("Validation errors:\n{}", all_errors.join("\n"));
    }
    assert!(
        all_errors.is_empty(),
        "{} block(s) failed validation",
        all_errors.len()
    );
}
