//! Go's `defer` support.
//!
//! A deferred cleanup guard that implements `Drop` to run cleanup at end of scope.

/// A deferred cleanup guard. Implements `Drop` to run cleanup at end of scope.
/// Usage: `let _defer = GoDeferGuard::new(|| { /* cleanup code */ });`
pub struct GoDeferGuard {
    cleanup: Box<dyn FnOnce()>,
}

impl GoDeferGuard {
    /// Creates a new defer guard with the given cleanup closure.
    pub fn new<F: FnOnce() + 'static>(f: F) -> Self {
        GoDeferGuard {
            cleanup: Box::new(f),
        }
    }
}

impl Drop for GoDeferGuard {
    fn drop(&mut self) {
        let cleanup = std::mem::replace(&mut self.cleanup, Box::new(|| {}));
        cleanup();
    }
}
