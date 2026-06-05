//! Go standard library package emulation.
//!
//! These modules emulate Go's standard library packages as runtime helpers.
//! Generated Go code calls these functions at compile time via `gourd::packages::*`.
//!
//! | Module | Go Package | Contents |
//! |--------|-----------|----------|
//! | `strings_impl` | `strings` | `strings_replace`, `strings_replace_all`, `has_prefix`, `has_suffix` |
//! | `strings_ops` | `strings` | `index`, `sort`, `reverse`, `contains`, `join`, `split`, `trim`, etc. |
//! | `strings` | `strings` | re-export from both above |
//! | `os_impl` | `os` | `os_open`, `os_read_file`, etc. |
//! | `os` | `os` | re-export from `os_impl` |
//! | `time_impl` | `time` | `time_now`, `time_since`, etc. |
//! | `time` | `time` | re-export from `time_impl` |
//! | `io_ops` | `io` | `io_copy`, `io_read_all` |
//! | `bytes_ops` | `bytes` | `bytes_contains`, `bytes_has_prefix`, etc. |
//! | `json_ops` | `json` | `json_marshal`, `json_unmarshal` |
//! | `math_ops` | `math` | `abs_i32`, `sqrt`, `floor`, etc. |
//! | `byte_ops` | — | `byte_of`, `rune_of`, etc. |

pub mod byte_ops;
pub mod bytes_ops;
pub mod io_ops;
pub mod json_ops;
pub mod math_ops;
pub mod os;
pub mod os_impl;
pub mod strings;
pub mod strings_impl;
pub mod strings_ops;
pub mod time;
pub mod time_impl;

// ─── Flat re-exports (for backward compatibility) ──────────────────────────

// Strings operations
pub use strings_ops::{index, slice_sub, sort, reverse, contains, join, split, contains_str, index_str, trim, trim_left, trim_right, to_upper, to_lower, repeat};
pub use strings_impl::{strings_replace, strings_replace_all, has_prefix, has_suffix, last_index_str, fields};

// OS operations
pub use os_impl::{os_open, os_read_file, os_write_file, os_mkdir, os_mkdir_all, os_remove, os_chdir, os_getenv, os_setenv, os_env_keys, os_args};

// I/O operations
pub use io_ops::{io_copy, io_read_all};

// Bytes operations
pub use bytes_ops::{bytes_contains, bytes_has_prefix, bytes_has_suffix, bytes_index, bytes_split, bytes_join, bytes_replace};

// JSON operations
pub use json_ops::{json_marshal, json_unmarshal};

// Time operations
pub use time_impl::{time_now, time_since, time_until, time_sleep};

// Math operations
pub use math_ops::{abs_i32, abs_i64, abs_f64, sqrt, floor, ceil, round, min_f64, max_f64, PI, E, exp, log, log10, pow, sign};

// Byte/rune operations
pub use byte_ops::{byte_of, rune_of, string_to_bytes, bytes_to_string};

#[cfg(test)]
mod tests;
