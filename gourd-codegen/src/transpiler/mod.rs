pub mod ast;
pub mod base_stmts;
pub mod control_flow;
pub mod expr;
pub mod free_fn;
pub mod funcs;
pub mod params;
pub mod parsing;
pub mod receiver;
pub mod return_stmts;
pub mod stmt_to_rust;
pub mod stmts;
pub mod slice_map;
pub mod switch;
pub mod types;

// Re-export the public API
#[allow(unused_imports)]
pub use free_fn::{go_to_rust_closure, go_to_rust_fn, go_to_rust_struct, go_to_rust_switch};
#[allow(unused_imports)]
pub use funcs::go_to_rust_receiver_fn;
