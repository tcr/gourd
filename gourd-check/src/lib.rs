//! `gourd-check` library for scanning and validating Go blocks.
//!
//! This crate provides utilities to:
//! - Extract `go!` blocks from Rust source files
//! - Validate extracted Go code using `go build`
//! - Report validation results

pub mod scanner;
pub mod validator;
pub mod report;
