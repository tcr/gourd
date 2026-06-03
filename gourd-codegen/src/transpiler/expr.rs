//! Expression-level Go → Rust transpilation.
//!
//! Converts `syn::Expr` AST nodes into `TokenStream` fragments via
//! recursive descent. Each `Expr` variant has a corresponding
//! `transpile_*` handler.
//!
//! Module layout:
//! - `dispatch.rs` — `go_to_rust()`, `go_to_rust_pattern()`, `emit_todo()`
//! - `literals.rs` — `Lit`, `Path`, `Paren`, `Array`, `Verbatim`
//! - `operators.rs` — `Binary`, `Unary`, `Cast`, `Assign`, `Break`
//! - `calls.rs` — `Call`, `MethodCall`, `Field`, `Index`, `Macro`
//! - `control_flow.rs` — `Let`, `Tuple`, `Return`, `Loop`, `ForLoop`,
//!   `While`, `Range`, `If`, `Block`

pub mod calls;
pub mod control_flow;
pub mod dispatch;
pub mod literals;
pub mod operators;
pub mod structs;

// Re-export the public entry points
pub use dispatch::{go_to_rust, go_to_rust_pattern};
