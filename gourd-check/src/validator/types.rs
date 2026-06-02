//! Type definitions for validation.
//!
//! Defines the core types used for Go and Rust validation results.

/// Result of formatting a validation check.
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
