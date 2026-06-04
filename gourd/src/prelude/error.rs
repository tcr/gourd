//! Go's `error` interface and error handling.
//!
//! Provides `GoError`, `make_error`, `check_error`, and `recover`.

/// Go's `error` interface — boxed error trait.
#[derive(Debug)]
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

/// Creates a new error (Go `errors.New()`).
pub fn make_error(message: &str) -> Box<dyn std::error::Error> {
    Box::new(GoError::new(message))
}

/// Panics if the error is not nil (Go `if err != nil { panic(err) }`).
pub fn check_error(err: Option<&dyn std::error::Error>) {
    if let Some(e) = err {
        panic!("Go runtime error: {}", e);
    }
}

/// Recovers from a panic and returns the panic payload as an error string.
/// Mirrors Go's `defer recover()` pattern.
///
/// Note: This is a placeholder. Actual recovery requires `std::panic::catch_unwind`
/// at the call site, not inside the `recover()` function.
pub fn recover() -> Option<String> {
    None
}
