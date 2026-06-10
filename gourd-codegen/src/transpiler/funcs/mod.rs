//! Receiver function output generation.
//!
//! Converts parsed `ReceiverFn` AST into Rust `impl` block tokens.

// Internal submodules — accessed by lib.rs but not re-exported publicly.
pub(crate) mod basic;
mod receiver;

// Re-export the public API
pub use basic::go_to_rust_receiver_fn;
pub use basic::go_to_rust_receiver_fn_hir;
