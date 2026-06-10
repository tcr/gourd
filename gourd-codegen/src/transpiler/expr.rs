//! Expression-level Go → Rust transpilation (legacy transition).
//!
//! Re-exports from legacy module for compatibility.

pub(crate) use super::legacy::expr_dispatch::{go_to_rust, go_to_rust_pattern};
pub(crate) use super::legacy::expr_calls;
pub(crate) use super::legacy::expr_closures;
pub(crate) use super::legacy::expr_control_flow;
pub(crate) use super::legacy::expr_literals;
pub(crate) use super::legacy::expr_operators;
pub(crate) use super::legacy::expr_structs;
