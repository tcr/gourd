use std::ops::Deref;
use std::sync::Arc;

#[derive(Debug)]
pub struct GoGc<T: 'static + ?Sized> {
    inner: Arc<T>,
}

impl<T: 'static> GoGc<T> {
    /// Allocates and wraps a value on the heap. Returns the only reference (refcount = 1).
    pub fn new(value: T) -> Self {
        GoGc {
            inner: Arc::new(value),
        }
    }

    /// Consumes the `GoGc<T>` and returns the inner `Arc<T>`. Reference count unchanged.
    pub fn into_inner(self) -> Arc<T> {
        self.inner
    }

    /// Unwraps to `T` if this is the last reference (refcount == 1).
    pub fn try_unwrap(self) -> Result<T, GoGc<T>> {
        Arc::try_unwrap(self.inner).map_err(|inner| GoGc { inner })
    }
}

impl<T: ?Sized> GoGc<T> {
    /// Returns the current strong reference count.
    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }
}

impl<T: ?Sized> Deref for GoGc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: ?Sized> Clone for GoGc<T> {
    fn clone(&self) -> Self {
        GoGc {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T: PartialEq + ?Sized> PartialEq for GoGc<T> {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl<T: Eq + ?Sized> Eq for GoGc<T> {}

impl<T: std::fmt::Display + ?Sized> std::fmt::Display for GoGc<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.inner, f)
    }
}
