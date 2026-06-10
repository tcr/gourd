//! Go source parsing: re-exports from modular sub-modules.
//!
//! This module formerly contained a 1,854-line monolith. It now delegates
//! to smaller, logically-organized files.

// Type definitions (re-exported for compatibility with existing code)
pub(crate) use super::hir::ast::*;

// Struct and Parse impl re-exports
pub(crate) use crate::transpiler::params::{GoFn, GoFnInputs, GoFnOutput, GoInterface, GoStruct};

// Statement parsing functions
pub(crate) use super::slice_map::{ElemParser, parse_go_slice_literal};

// Statement-to-Rust conversion
pub(crate) use super::legacy::stmt_to_rust::go_stmt_to_rust;
