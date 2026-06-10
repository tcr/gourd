//! Free function and struct transpilation (legacy path).
//!
//! Converts Go function declarations (`fn name() { ... }`) and struct
//! declarations (`struct Name { field type }`) into Rust.
//!
//! This is the legacy bridge — used only by `lib.rs:transpile_go` for
//! free functions that don't have a receiver group. The HIR path handles
//! everything else.

pub(crate) mod basic;

// Re-export the public API (used by lib.rs entry point)
pub use basic::{go_to_rust_fn, go_to_rust_struct};
