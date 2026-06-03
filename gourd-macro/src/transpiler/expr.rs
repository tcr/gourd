//! Expression-level Go → Rust transpilation.
//!
//! Re-exported from `gourd-codegen` for public API access.

pub use gourd_codegen::transpiler::expr::{
    calls, control_flow, dispatch, literals, operators, emit_todo,
    go_to_rust, go_to_rust_pattern,
};
