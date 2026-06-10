//! Free function and struct transpilation.
//!
//! Converts Go function declarations (`fn name() { ... }`) and struct
//! declarations (`struct Name { field type }`) into Rust.

// Internal submodules — accessed by lib.rs but not re-exported publicly.
pub(crate) mod basic;
pub(crate) mod closure;
pub(crate) mod interface;
pub(crate) mod select;
pub(crate) mod switch;
mod util;

// Re-export the public API
pub use basic::{go_to_rust_fn, go_to_rust_struct, go_to_rust_fn_hir, go_to_rust_struct_hir};
pub use closure::go_to_rust_closure;
pub use interface::{go_to_rust_interface, go_to_rust_interface_hir};
pub use util::to_snake_case;
// Re-export types for consumers that need Go AST types.
// These are public through the transpiler module.
