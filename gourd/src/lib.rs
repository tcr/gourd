//! gourd runtime: Go-style memory management via Arc-based reference counting.
//!
//! Provides [`GoGc<T>`], a lightweight wrapper around `Arc<T>` that mirrors
//! Go's garbage-collected pointer semantics (heap-allocated, shared ownership,
//! automatic deallocation when the last reference is dropped).
//!
//! ## Module organization
//!
//! - `gourd::GoGc<T>` — root-level GC pointer
//! - `gourd::GoScheduler` — root-level task scheduler (crossbeam-based)
//! - `gourd::GoChannel<T>` — root-level channel (crossbeam-based)
//! - `gourd::GoSelect<T>` — root-level select (crossbeam-based)
//! - `gourd::prelude::*` — runtime types (GoMutex, GoRc, GoError, etc.)
//! - `gourd::packages::*` — package emulation (os, strings, json, etc.)

mod go_gc;
pub use go_gc::GoGc;

mod go_scheduler;
pub use go_scheduler::*;

/// Re-export the declaration macro for Go declarations.
pub use gourd_macro::go;

/// Compile-time verification attribute for Go declarations.
pub use gourd_macro::verify_rust_output;

/// Transpile a Go declaration to Rust tokens (programmatic access).
pub use gourd_codegen::{transpile_go, transpile_go_text};

/// Source-level scanner for `go!` blocks and `#[verify_rust_output]` attributes.
pub mod scanner;

/// Go-style runtime prelude (modules only, no duplicate runtime types).
pub mod prelude;

/// Go stdlib package emulation (os, strings, json, etc.).
pub mod packages;
