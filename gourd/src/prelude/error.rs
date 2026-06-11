//! Go's `error` interface and error handling.
//!
//! Provides `GoError`, panic value storage, and `recover()`.

use std::cell::RefCell;

thread_local! {
    /// Thread-local panic value slot.
    /// Stores the panic payload when a Go function panics and is caught
    /// by `catch_unwind`. `recover()` reads from this slot.
    static PANIC_VALUE: RefCell<Option<Box<dyn std::error::Error + Send>>> = const { RefCell::new(None) };
}

/// Go's `error` interface — boxed error trait.
#[derive(Debug, PartialEq, Eq)]
pub struct GoError {
    message: String,
}

impl GoError {
    /// Creates a new error with the given message.
    pub fn new(message: &str) -> Self {
        GoError {
            message: message.to_string(),
        }
    }

    /// Returns the error message (Go's `Error()` method).
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for GoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for GoError {}

/// Set the panic value in the thread-local slot.
/// Called by `go_with_panic_slot` when `catch_unwind` catches a panic.
pub fn set_panic_value(e: Box<dyn std::error::Error + Send>) {
    PANIC_VALUE.with(|slot| {
        *slot.borrow_mut() = Some(e);
    });
}

/// Recovers from a panic and returns the panic payload as an error string.
/// Mirrors Go's `defer recover()` pattern.
///
/// Requires that the calling code was wrapped in `go_with_panic_slot`.
pub fn recover() -> Option<GoError> {
    PANIC_VALUE.with(|slot| {
        slot.borrow_mut().take().map(|e| GoError { message: e.to_string() })
    })
}

/// Wraps a closure in `catch_unwind`, storing any panic value in the
/// thread-local slot for `recover()` to read.
///
/// Usage:
/// ```ignore
/// let result = go_with_panic_slot(|| {
///     panic!("something went wrong");
///     42
/// });
/// // result is Err(GoError { message: "something went wrong" })
/// let recovered = recover(); // Some(GoError { ... })
/// ```
pub fn go_with_panic_slot<F, R>(f: F) -> Result<R, GoError>
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    match std::panic::catch_unwind(f) {
        Ok(result) => Ok(result),
        Err(payload) => {
            let msg = if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = payload.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "panic()".to_string()
            };
            Err(GoError::new(&msg))
        }
    }
}
