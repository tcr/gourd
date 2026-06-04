//! Go's `any` / `interface{}` type.
//!
//! Can hold any type. Used for generic values that can hold arbitrary types.

/// Go's `any` / `interface{}` — can hold any type.
///
/// This is the runtime equivalent of Go's untyped interface.
/// Used by the transpiler for generic values that can hold arbitrary types.
///
/// ```
/// use gourd::prelude::Any;
///
/// let a: Any = Any::from(42i32);
/// let b: Any = Any::from("hello");
/// assert_eq!(a.downcast_ref::<i32>(), Some(&42));
/// assert_eq!(b.downcast_ref::<&str>(), Some(&"hello"));
/// ```
#[derive(Debug)]
pub struct Any {
    inner: Box<dyn std::any::Any + Send + Sync>,
    type_name: &'static str,
}

impl Any {
    /// Creates a new `Any` holding the given value.
    pub fn from<T: 'static + Send + Sync>(value: T) -> Self {
        Any {
            inner: Box::new(value),
            type_name: std::any::type_name::<T>(),
        }
    }

    /// Downcasts to a reference of type `T`. Returns `None` if the type
    /// does not match.
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.inner.downcast_ref::<T>()
    }

    /// Downcasts to a mutable reference of type `T`. Returns `None` if
    /// the type does not match.
    pub fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.inner.downcast_mut::<T>()
    }

    /// Consumes the `Any` and returns the inner value of type `T`.
    /// Returns `None` if the type does not match.
    pub fn downcast<T: 'static>(self) -> Option<T> {
        match self.inner.downcast::<T>() {
            Ok(boxed) => Some(*boxed),
            Err(_) => None,
        }
    }

    /// Returns the type name of the held value.
    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// Returns true if the held value is of type `T`.
    pub fn is<T: 'static>(&self) -> bool {
        self.inner.is::<T>()
    }
}

// Clone is intentionally NOT implemented for Any. This prevents
// silent type erasure — users must explicitly downcast.
// If needed in the future, we can implement it by boxing the inner type.
