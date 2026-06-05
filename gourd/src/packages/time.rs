//! Go's `time` package helpers.
//!
//! Re-exports time functions from `time_impl` for use via explicit `import`
//! statements.
//!
//! Import path: `use gourd::packages::time::*;`
//! Used by: `import time`

pub use super::time_impl::*;
