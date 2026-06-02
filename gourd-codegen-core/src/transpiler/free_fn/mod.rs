//! Free function and struct transpilation.
//!
//! Converts Go function declarations (`fn name() { ... }`) and struct
//! declarations (`struct Name { field type }`) into Rust.

mod basic;
mod interface;
mod switch;

// Re-export the public API
pub use basic::{go_to_rust_fn, go_to_rust_struct};
pub use interface::go_to_rust_interface;
pub use switch::{go_to_rust_switch, transpile_switch};
mod util;
