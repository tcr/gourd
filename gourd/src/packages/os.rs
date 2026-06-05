//! Go's `os` package helpers.
//!
//! Re-exports OS functions from `os_impl` for use via explicit `import`
//! statements.
//!
//! Import path: `use gourd::packages::os::*;`
//! Used by: `import os`

pub use super::os_impl::*;
