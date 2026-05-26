//! gourd runtime: Go-style memory management via Arc-based reference counting.
//!
//! Provides [`GoGc<T>`], a lightweight wrapper around `Arc<T>` that mirrors
//! Go's garbage-collected pointer semantics (heap-allocated, shared ownership,
//! automatic deallocation when the last reference is dropped).

mod go_gc;
pub use go_gc::GoGc;

/// Re-export the expression macro for inline Go transpilation.
pub use gourd_codegen::go_expr;

/// Re-export the declaration macro for Go declarations.
pub use gourd_codegen::go;
