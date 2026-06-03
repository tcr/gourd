//! Free function and struct transpilation.
//!
//! Converts Go function declarations (`fn name() { ... }`) and struct
//! declarations (`struct Name { field type }`) into Rust.

mod basic;
mod closure;
mod interface;
pub(crate) mod select;
mod switch;
mod util;

// Re-export the public API
pub use basic::{go_to_rust_fn, go_to_rust_struct};
pub use closure::go_to_rust_closure;
pub use interface::go_to_rust_interface;
pub use select::{go_to_rust_select, go_to_rust_select_ast};
pub(crate) use select::parse_select_body;
pub use switch::{go_to_rust_switch, transpile_switch};
