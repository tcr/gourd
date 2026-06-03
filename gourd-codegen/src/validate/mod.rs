//! Validation functions.
//!
//! Validates Go and Rust code by running real compilers.

mod temp;
mod go;
mod rust;
mod helpers;

// Re-export the public API
pub use go::validate_go;
pub use rust::validate_rust;
