//! Scanner module — delegates to `gourd-codegen` scanner.

use anyhow::Result;
use std::path::Path;

pub use gourd_codegen::scanner::{GoBlock, VerifyBlock};

/// Scan a path for `go!` blocks, skipping gourd-check's own source.
pub fn scan_path(path: &Path) -> Result<Vec<GoBlock>> {
    let config = gourd_codegen::scanner::ScanConfig::default()
        .with_skip_components(vec!["gourd-check", "src"]);
    gourd_codegen::scanner::scan_path_with_config(path, &config)
        .map_err(|e| anyhow::anyhow!("{}", e))
}

/// Scan a path for `#[verify_rust_output]` attributes, skipping gourd-check's own source.
pub fn scan_verify(path: &Path) -> Result<Vec<VerifyBlock>> {
    let config = gourd_codegen::scanner::ScanConfig::default()
        .with_skip_components(vec!["gourd-check", "src"]);
    gourd_codegen::scanner::scan_verify_with_config(path, &config)
        .map_err(|e| anyhow::anyhow!("{}", e))
}
