//! Validation functions and types.
//!
//! Validates Go blocks by running `go build` and Rust blocks by
//! running `cargo check`.

pub mod types;
pub mod normalize;
pub mod validate;
pub mod temp;

// Re-export the public API
pub use types::{FormatResult, CheckResult, Validation, VerifyCheck};
pub use normalize::{check_results_to_format, verify_checks_to_format, normalize_go_code};
pub use validate::{validate_verify_block, validate_verify_blocks, validate_go};
