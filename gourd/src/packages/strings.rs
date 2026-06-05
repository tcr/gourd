//! Go's `strings` package helpers.
//!
//! Re-exports from both `strings_impl` (original strings functions) and
//! `strings_ops` (array/string ops) for use via explicit `import` statements.
//!
//! Import path: `use gourd::packages::strings::*;`
//! Used by: `import strings`

pub use super::strings_impl::*;
pub use super::strings_ops::*;
