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

impl<T: ?Sized> GoGc<T> {
    /// Attempts to get a mutable reference to the inner value.
    /// Returns None if there are other references to this GoGc.
    pub fn get_mut(&mut self) -> Option<&mut T> {
        Arc::get_mut(&mut self.inner)
    }
}

impl<T: Sized + 'static> GoGc<T> {
    /// Replace the inner value wholesale.
    pub fn replace(&mut self, value: T) {
        self.inner = Arc::new(value);
    }
}

impl<T: Clone + 'static> GoGc<Vec<T>> {
    /// Reserve capacity in the backing Vec.
    /// If we own the Arc (refcount == 1), reserves in place.
    /// Otherwise, clones, reserves, and replaces the Arc.
    pub fn reserve(&mut self, additional: usize) {
        if let Some(v) = Arc::get_mut(&mut self.inner) {
            v.reserve(additional);
        } else {
            let mut new = (**self).clone();
            new.reserve(additional);
            self.inner = Arc::new(new);
        }
    }

    /// Push an item into the backing Vec.
    /// If we own the Arc (refcount == 1), pushes in place.
    /// Otherwise, clones, pushes, and replaces the Arc.
    pub fn push(&mut self, item: T) {
        if let Some(v) = Arc::get_mut(&mut self.inner) {
            v.push(item);
        } else {
            let mut new = (**self).clone();
            new.push(item);
            self.inner = Arc::new(new);
        }
    }

    /// Extend the backing Vec with items.
    /// If we own the Arc (refcount == 1), extends in place.
    /// Otherwise, clones, extends, and replaces the Arc.
    pub fn extend_from_slice(&mut self, items: &[T]) {
        if let Some(v) = Arc::get_mut(&mut self.inner) {
            v.extend_from_slice(items);
        } else {
            let mut new = (**self).clone();
            new.extend_from_slice(items);
            self.inner = Arc::new(new);
        }
    }
}

impl<T: Clone + Default + 'static> GoGc<Vec<T>> {
    /// Set an element at the given index in the backing Vec.
    /// If the Vec is too short, extends with default values up to idx.
    /// If we own the Arc (refcount == 1), sets in place.
    /// Otherwise, clones, sets, and replaces the Arc.
    pub fn set_at(&mut self, idx: usize, value: T) {
        let extend_to = idx + 1;
        if let Some(v) = Arc::get_mut(&mut self.inner) {
            if v.len() < extend_to {
                v.resize(extend_to, T::default());
            }
            v[idx] = value;
        } else {
            let mut new = (**self).clone();
            if new.len() < extend_to {
                new.resize(extend_to, T::default());
            }
            new[idx] = value;
            self.inner = Arc::new(new);
        }
    }
}

impl<T: 'static + Clone> GoGc<Vec<T>> {
    /// Converts this GoGc<Vec<T>> into a Vec<T>.
    /// If refcount == 1, returns the Arc's Vec directly.
    /// Otherwise, returns a cloned copy.
    pub fn into_vec(self) -> Vec<T> {
        Arc::try_unwrap(self.inner).unwrap_or_else(|arc| (*arc).clone())
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
