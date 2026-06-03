//! Temporary directory functions.
//!
//! Provides a function for creating temporary directories for validation.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// Create a temporary directory with a unique name.
pub(crate) fn temp_dir(prefix: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{}_{}_{:x}", prefix, std::process::id(), n))
}
