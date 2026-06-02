//! Receiver function output generation.
//!
//! Converts parsed `ReceiverFn` AST into Rust `impl` block tokens.

mod basic;
mod receiver;

// Re-export the public API
pub use basic::go_to_rust_receiver_fn;
