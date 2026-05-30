pub mod expr;
pub mod free_fn;
pub mod funcs;
pub mod parsing;
pub mod types;

// Re-export the public API
pub use free_fn::{go_to_rust_fn, go_to_rust_struct, go_to_rust_switch};

