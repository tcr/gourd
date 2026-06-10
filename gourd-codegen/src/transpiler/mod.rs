//! Go → Rust transpiler module.
//!
//! This module contains the core transpilation logic:
//! - `hir/` — High-level intermediate representation (preferred path)
//! - `legacy/` — Legacy transition layer for low-level primitives
//! - Supporting utilities: types, slice_map, parsing, free_fn

// Type mapping utilities (used by legacy transpilation primitives)
pub(crate) mod types;

// Function parameter parsing
pub(crate) mod params;

// Map/slice literal parsing (used by legacy transpilation primitives)
pub(crate) mod slice_map;

// Statement parsing and type declarations
pub(crate) mod parsing;

// Heuristic detection — variable-name-based guesses when type info is unavailable.
// These are inherently unreliable and should be removed over time as type information
// becomes available through better analysis.
pub(crate) mod heuristics;

// Top-level declarations (free functions, structs, interfaces)
pub(crate) mod free_fn;

// HIR module (preferred path for new code)
pub mod hir;

// Transition layer — old code kept as compatibility for the HIR pipeline
pub(crate) mod legacy;

// Re-export HIR public API only
pub use crate::transpiler::hir::*;
