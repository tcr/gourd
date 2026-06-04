/// Helper module for conditional debug output based on GOURD_DEBUG env var.
///
/// When the `GOURD_DEBUG` environment variable is set (to any value),
/// debug messages are printed to stderr.

/// Returns true if GOURD_DEBUG is set.
pub fn enabled() -> bool {
    std::env::var("GOURD_DEBUG").is_ok()
}

/// Print a debug message to stderr if GOURD_DEBUG is set.
#[macro_export]
macro_rules! debug_println {
    ($($arg:tt)*) => {
        if $crate::debug::enabled() {
            eprintln!($($arg)*);
        }
    };
}
