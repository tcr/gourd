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
//! | `os_impl` | `os_open`, `os_read_file`, `os_write_file`, `os_mkdir`, etc. |
//! | `strings_impl` | `strings_replace`, `strings_replace_all`, `has_prefix`, `has_suffix`, etc. |
//! | `strings_ops` | `index`, `join`, `split`, `trim`, `contains`, etc. |
//! | `io_ops` | `io_copy`, `io_read_all` |
//! | `bytes_ops` | `bytes_contains`, `bytes_has_prefix`, `bytes_has_suffix`, etc. |
//! | `json_ops` | `json_marshal`, `json_unmarshal` |
//! | `time_impl` | `time_now`, `time_since`, `time_sleep`, `time_until` |
//! | `byte_ops` | `byte_of`, `rune_of`, `string_to_bytes`, `bytes_to_string` |
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

// ─── Runtime primitives ────────────────────────────────────────────────────

pub mod any;
pub mod defer_guard;
pub mod error;
pub mod fmt_ops;
pub mod rand;
pub mod std;
pub mod sync;

// ─── Package emulation (re-exported from packages module) ──────────────────

// Re-export all package functions so generated code using
// `::gourd::prelude::os_open(...)` etc. works.
pub use crate::packages::{
    // Strings operations
    index, join, slice_sub, sort, reverse, contains, split,
    contains_str, index_str, trim, trim_left, trim_right, to_upper, to_lower, repeat, fields,
    // Strings helpers
    strings_replace, strings_replace_all, has_prefix, has_suffix, last_index_str,
    // OS operations
    os_open, os_read_file, os_write_file, os_mkdir, os_mkdir_all, os_remove,
    os_chdir, os_getenv, os_setenv, os_env_keys, os_args,
    // I/O operations
    io_copy, io_read_all,
    // Bytes operations
    bytes_contains, bytes_has_prefix, bytes_has_suffix, bytes_index,
    bytes_split, bytes_join, bytes_replace,
    // JSON operations
    json_marshal, json_unmarshal,
    // Time operations
    time_now, time_since, time_until, time_sleep,
    // Byte/rune operations
    byte_of, rune_of, string_to_bytes, bytes_to_string,
    // Math operations
    abs_i32, abs_i64, abs_f64, sqrt, floor, ceil, round, min_f64, max_f64,
    PI, E, exp, log, log10, pow, sign,
};

// ─── Prelude re-exports ────────────────────────────────────────────────────

// Memory management
pub use defer_guard::GoDeferGuard;

// Synchronization
pub use sync::{GoMutex, GoMutexGuard, GoRc, GoOnce, GoOnceArgs, GoWaitGroup, GoRWMutex, GoRwReadGuard, GoRwWriteGuard};

// Error handling
pub use error::{GoError, make_error, check_error, recover};

// Any (interface{})
pub use any::Any;

// Standard library builtins
pub use std::{len, cap, append, make_slice, copy, min, max, std_copy, std_copy_slice, std_append};
pub use ::std::collections::HashMap;

// Deprecated: map helper functions (use GoMap::get/set/delete instead)
pub use std::{make_map, map_get, map_get_ref, map_set_mut, map_set_mut_ref, map_set_val, display_map, std_delete};

// Random
pub use rand::GoRand;

// Formatting
pub use fmt_ops::{fmt_sprintf, fmt_print, fmt_println, fmt_printf, fmt_print_vec, fmt_println_vec};
