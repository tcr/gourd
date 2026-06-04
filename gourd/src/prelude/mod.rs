//! Gourd prelude: standard library of Go constructs.
//!
//! This module provides the runtime types and functions that correspond to
//! Go's standard library. Generated code references these items at compile
//! time via `gourd::prelude::*`.
//!
//! # Module organization
//!
//! The prelude is organized into submodules by Go package grouping:
//!
//! | Submodule | Contents |
//! |-----------|----------|
//! | `defer_guard` | `GoDeferGuard` — `defer` support |
//! | `sync` | `GoMutex`, `GoMutexGuard`, `GoRc`, `GoOnce`, `GoWaitGroup`, `GoRWMutex` |
//! | `fmt_ops` | `fmt_sprintf`, `fmt_print`, `fmt_println`, `fmt_printf` |
//! | `rand` | `GoRand` — pseudo-random numbers |
//! | `error` | `GoError`, `make_error`, `check_error`, `recover` |
//! | `any` | `Any` — Go's `interface{}` |
//! | `std` | `len`, `cap`, `append`, `make_slice`, `make_map`, `copy`, `min`, `max` |
//!
//! # Root-level types (not in prelude)
//!
//! Some Go runtime primitives are exported at the crate root level because
//! the transpiler generates code that imports them directly:
//!
//! - `gourd::GoGc<T>` — garbage-collected pointer
//! - `gourd::GoScheduler` — task scheduler
//! - `gourd::GoChannel<T>` — typed channel
//! - `gourd::GoSelect<T>` — channel multiplexing
//! - `gourd::SchedulerMap` — multi-scheduler map
//! - `gourd::GoFuture` — closure-as-future
//! - `gourd::GoGc<T>` — GC pointer (re-exported from root)

pub mod any;
pub mod defer_guard;
pub mod error;
pub mod fmt_ops;
pub mod rand;
pub mod std;
pub mod sync;

// NOTE: Package emulation is in `gourd::packages::*`, not here.
// NOTE: Go runtime primitives (GoGc, GoScheduler, GoChannel, etc.) are
// exported at the crate root level, not in the prelude.

// ─── Re-exports ────────────────────────────────────────────────────────────

// Memory management
pub use defer_guard::GoDeferGuard;

// Synchronization
pub use sync::{GoMutex, GoMutexGuard, GoRc, GoOnce, GoOnceArgs, GoWaitGroup, GoRWMutex, GoRwReadGuard, GoRwWriteGuard};

// Error handling
pub use error::{GoError, make_error, check_error, recover};

// Any (interface{})
pub use any::Any;

// Standard library builtins
pub use std::{len, cap, append, make_slice, make_map, copy, min, max, std_copy, std_delete, std_append};

// Random
pub use rand::GoRand;

// Formatting
pub use fmt_ops::{fmt_sprintf, fmt_print, fmt_println, fmt_printf};
