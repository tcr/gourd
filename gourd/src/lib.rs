//! gourd runtime: Go-style memory management via Arc-based reference counting.
//!
//! Provides [`GoGc<T>`], a lightweight wrapper around `Arc<T>` that mirrors
//! Go's garbage-collected pointer semantics (heap-allocated, shared ownership,
//! automatic deallocation when the last reference is dropped).

mod go_gc;
pub use go_gc::GoGc;

/// Re-export the declaration macro for Go declarations.
pub use gourd_codegen::go;

/// Compile-time verification attribute for Go declarations.
pub use gourd_codegen::verify_rust_output;

/// Transpile a Go declaration to Rust tokens (programmatic access).
pub use gourd_codegen_core::transpile_go;

