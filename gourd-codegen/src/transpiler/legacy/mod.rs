//! Legacy transition layer — old code moved here for the HIR pipeline.
//! This directory contains all pre-HIR transpilation primitives that HIR delegates to.
//! Files are kept flat for simplicity.

pub(crate) mod base_stmts;
pub(crate) mod control_flow;
pub(crate) mod expr_calls;
pub(crate) mod expr_closures;
pub(crate) mod expr_control_flow;
pub(crate) mod expr_dispatch;
pub(crate) mod expr_literals;
pub(crate) mod expr_operators;
pub(crate) mod expr_structs;
pub(crate) mod return_stmts;
pub(crate) mod stmt_to_rust;
pub(crate) mod stmts;

// Public API for HIR delegation
pub(crate) use base_stmts::parse_base_stmt;
pub(crate) use control_flow::{parse_go_for, parse_go_if, parse_go_while};
pub(crate) use return_stmts::parse_go_return;
pub(crate) use stmt_to_rust::{go_stmt_to_rust, transpile_go_body};
pub(crate) use expr_dispatch::{go_to_rust, go_to_rust_pattern};
