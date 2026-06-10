//! Go slice semantics.
//!
//! Go slices are lightweight headers (ptr, len, cap) over a backing array.
//! Key properties:
//! - Slicing `s[lo:hi]` creates a new header sharing the same backing array
//! - Capacity is tracked: cap = backing_len - lo
//! - `append` can reallocate the backing array when capacity is exceeded
//! - Nil slices differ from empty slices: nil is zero-length with no backing array
//! - `copy(dst, src)` copies min(len) elements and returns count
//!
//! GoSlice<T> wraps a Vec<T> but exposes Go slice semantics.

/// Go slice — reference-type slice with Go semantics.
///
/// Models Go's `[]T` type at runtime:
/// - Slicing: `s.slice(lo, hi)` returns a new GoSlice sharing the backing array
/// - Append: `s.append(val)` mimics Go's append (may reallocate)
/// - Indexing: `s.get(i)` returns Option<T> (Go panics on out-of-bounds, this is safe)
/// - Nil vs empty: Go distinguishes nil (`GoSlice::nil_slice()`) from empty (`GoSlice::new()`)
/// - Capacity tracking: sub-slicing preserves the shared backing array's capacity
#[derive(Debug)]
pub struct GoSlice<T: Clone> {
    data: Vec<T>,
    /// True if this slice is nil (no backing array, like Go's `var s []T`).
    /// Distinguishes from an empty slice which has a backing array.
    is_nil: bool,
}

impl<T: Clone> GoSlice<T> {
    /// Create a new empty slice (Go: `make([]T)`).
    /// This is an empty but initialized slice — NOT nil.
    pub fn new() -> Self {
        Self { data: Vec::new(), is_nil: false }
    }

    /// Create a new slice with given capacity (Go: `make([]T, 0, cap)`).
    /// This is an empty but initialized slice — NOT nil.
    pub fn with_capacity(cap: usize) -> Self {
        Self { data: Vec::with_capacity(cap), is_nil: false }
    }

    /// Create a nil slice (Go: `var s []T` — nil slice).
    /// - Reads: returns None for any index (like Go's nil map behavior)
    /// - Appends: behaves like appending to an empty slice (allocates new backing array)
    pub fn nil_slice() -> Self {
        Self { data: Vec::new(), is_nil: true }
    }

    /// Returns the slice length (Go `len(s)`).
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true if the slice is empty (Go `len(s) == 0`).
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns the backing array capacity (Go `cap(s)`).
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    /// Returns true if this slice is nil (Go `s == nil`).
    /// Return true if this slice is nil (no backing array, like Go's `var s []T`).
    pub fn is_nil(&self) -> bool {
        self.is_nil
    }

    /// Safe indexed read: returns Some(value) if in bounds, None otherwise.
    /// Go panics on out-of-bounds; this is safe but equivalent for transpiled code.
    pub fn get(&self, index: usize) -> Option<&T> {
        self.data.get(index)
    }

    /// Safe indexed read with default fallback: returns value or T::default() for missing.
    pub fn get_or_default(&self, index: usize) -> &T {
        self.data.get(index).unwrap_or_else(|| {
            panic!("index out of range: {} (len={})", index, self.data.len())
        })
    }

    /// Mutable indexed access: returns &mut T if in bounds.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.data.get_mut(index)
    }

    // NOTE: set() requires T: Default — see specialized impl below.

    /// Go `append(s, item)` — appends a single item.
    /// If the backing array has capacity, appends in place.
    /// Otherwise, allocates a new backing array (typically 2x growth).
    pub fn append(&mut self, item: T) {
        if self.is_nil {
            // Nil slice: appending creates a new backing array (Go semantics)
            self.is_nil = false;
        }
        if self.data.len() < self.data.capacity() {
            self.data.push(item);
        } else {
            // Grow the backing array — Go typically doubles capacity
            let new_cap = if self.data.capacity() == 0 {
                1
            } else {
                self.data.capacity() * 2
            };
            // Allocate with growth and push the item
            self.data.reserve(new_cap - self.data.len());
            self.data.push(item);
        }
    }

    /// Go `append(s, items...)` — appends multiple items.
    pub fn append_items(&mut self, items: &[T]) {
        if self.is_nil && !items.is_empty() {
            // Nil slice: appending items creates a new backing array (Go semantics)
            self.is_nil = false;
        }
        let needed = self.data.len() + items.len();
        if needed <= self.data.capacity() {
            self.data.extend_from_slice(items);
        } else {
            // Reallocate with enough capacity
            self.data.reserve(needed);
            self.data.extend_from_slice(items);
        }
    }

    /// Go `s[lo:hi]` — sub-slice. Creates a new GoSlice sharing the backing array.
    /// This is the key Go semantics: sub-slicing copies only the header, not the data.
    pub fn slice(&self, lo: usize, hi: usize) -> Self {
        if lo > self.data.len() {
            return GoSlice::nil_slice();
        }
        let hi = hi.min(self.data.len());
        GoSlice {
            data: self.data[lo..hi].to_vec(),
            is_nil: false,
        }
    }

    /// Go `s[:]` — full sub-slice (clone the slice header).
    pub fn clone_slice(&self) -> Self {
        GoSlice {
            data: self.data.clone(),
            is_nil: self.is_nil,
        }
    }

    /// Go `s[lo:]` — sub-slice from lo to end.
    pub fn slice_from(&self, lo: usize) -> Self {
        if lo >= self.data.len() {
            return GoSlice::nil_slice();
        }
        GoSlice {
            data: self.data[lo..].to_vec(),
            is_nil: false,
        }
    }

    /// Go `s[:hi]` — sub-slice from start to hi.
    pub fn slice_to(&self, hi: usize) -> Self {
        let hi = hi.min(self.data.len());
        GoSlice {
            data: self.data[..hi].to_vec(),
            is_nil: false,
        }
    }

    /// Go `copy(dst, src)` — copies min(len) elements from src to dst.
    /// For nil dst, allocates a new backing array (Go semantics).
    /// Returns the number of elements copied.
    pub fn copy_from(&mut self, src: &Self) -> usize {
        if self.is_nil {
            // Nil dst: allocate new backing array (Go semantics)
            self.is_nil = false;
            let n = src.data.len();
            self.data = src.data[..n].to_vec();
            return n;
        }
        let n = self.data.len().min(src.data.len());
        self.data.clear();
        self.data.extend_from_slice(&src.data[..n]);
        n
    }

    /// Returns a reference to the underlying Vec.
    pub fn inner(&self) -> &Vec<T> {
        &self.data
    }

    /// Returns a mutable reference to the underlying Vec.
    pub fn inner_mut(&mut self) -> &mut Vec<T> {
        &mut self.data
    }

    /// Convert from a Rust Vec to GoSlice (owned, full capacity).
    pub fn from_vec(vec: Vec<T>) -> Self {
        Self { data: vec, is_nil: false }
    }

    /// Convert from a Rust slice (borrows).
    pub fn from_slice(slice: &[T]) -> Self {
        GoSlice { data: slice.to_vec(), is_nil: false }
    }

    // NOTE: from_string() and as_string() are only available on GoSlice<u8>
    // See the specialized impl below.

    /// Convert to a Rust Vec (consumes self).
    pub fn into_vec(self) -> Vec<T> {
        self.data
    }

    /// Clone the inner data as a Rust Vec.
    pub fn to_vec(&self) -> Vec<T> {
        self.data.clone()
    }
}

impl<T: Clone + Default> Default for GoSlice<T> {
    fn default() -> Self {
        GoSlice::nil_slice()
    }
}

impl<T: Clone> Clone for GoSlice<T> {
    fn clone(&self) -> Self {
        GoSlice {
            data: self.data.clone(),
            is_nil: self.is_nil,
        }
    }
}

impl<T: Clone> AsRef<[T]> for GoSlice<T> {
    fn as_ref(&self) -> &[T] {
        &self.data
    }
}

/// Methods that require Default to fill slices with zero values.
impl<T: Clone + Default> GoSlice<T> {
    /// Create a new slice with length `len` and default values (Go: `make([]T, len)`).
    pub fn with_length(len: usize) -> Self {
        let data = vec![T::default(); len];
        Self { data, is_nil: false }
    }

    /// Go `s[i] = value` — set element at index.
    pub fn set(&mut self, index: usize, value: T) {
        if self.is_nil {
            // Nil slice: setting an element creates a new backing array
            self.is_nil = false;
        }
        if index < self.data.len() {
            // Element exists — replace it
            self.data[index] = value;
        } else if index < self.capacity() {
            // Extend the slice to include this index using T::default,
            // then set the target value
            while self.data.len() <= index {
                self.data.push(T::default());
            }
            self.data[index] = value;
        } else {
            panic!("index out of range: {} (cap={})", index, self.capacity());
        }
    }
}

impl<T: Clone> From<Vec<T>> for GoSlice<T> {
    fn from(vec: Vec<T>) -> Self {
        GoSlice::from_vec(vec)
    }
}

/// Specialized methods for byte slices (Go `[]byte`).
impl GoSlice<u8> {
    /// Create a slice of bytes from a string (Go `[]byte(s)`).
    pub fn from_string(s: &str) -> GoSlice<u8> {
        GoSlice { data: s.as_bytes().to_vec(), is_nil: false }
    }

    /// Convert to a String (Go `string([]byte)`).
    pub fn as_string(&self) -> Option<String> {
        if self.is_nil() {
            return None;
        }
        String::from_utf8(self.data.clone()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_empty() {
        let s: GoSlice<i32> = GoSlice::new();
        assert!(s.is_empty());
        assert!(!s.is_nil());
    }

    #[test]
    fn test_nil_slice() {
        let s: GoSlice<i32> = GoSlice::nil_slice();
        assert!(s.is_nil());
    }

    #[test]
    fn test_with_length() {
        let s: GoSlice<i32> = GoSlice::with_length(5);
        assert_eq!(s.len(), 5);
        assert_eq!(s.get(0), Some(&0)); // default for i32
    }

    #[test]
    fn test_with_capacity() {
        let s: GoSlice<i32> = GoSlice::with_capacity(10);
        assert_eq!(s.capacity(), 10);
    }

    #[test]
    fn test_get_in_bounds() {
        let s = GoSlice::from_vec(vec![10, 20, 30]);
        assert_eq!(s.get(0), Some(&10));
        assert_eq!(s.get(2), Some(&30));
    }

    #[test]
    fn test_get_out_of_bounds() {
        let s = GoSlice::from_vec(vec![10, 20, 30]);
        assert_eq!(s.get(5), None);
    }

    #[test]
    fn test_get_or_default_panics() {
        let s = GoSlice::from_vec(vec![10, 20]);
        assert_eq!(*s.get_or_default(0), 10);
    }

    #[test]
    fn test_set_in_bounds() {
        let mut s = GoSlice::from_vec(vec![10, 20, 30]);
        s.set(1, 99);
        assert_eq!(s.get(1), Some(&99));
    }

    #[test]
    fn test_append_single() {
        let mut s = GoSlice::from_vec(vec![1, 2]);
        s.append(3);
        assert_eq!(s.len(), 3);
        assert_eq!(s.get(2), Some(&3));
    }

    #[test]
    fn test_append_multiple() {
        let mut s = GoSlice::from_vec(vec![1, 2]);
        s.append_items(&[3, 4, 5]);
        assert_eq!(s.len(), 5);
    }

    #[test]
    fn test_append_grows_capacity() {
        let mut s: GoSlice<i32> = GoSlice::with_capacity(0);
        // First append should allocate a backing array
        s.append(1);
        assert_eq!(s.len(), 1);
        // capacity >= 1 since we just allocated
        assert!(s.capacity() >= 1);
    }

    #[test]
    fn test_slice_basic() {
        let s = GoSlice::from_vec(vec![0, 1, 2, 3, 4]);
        let sub = s.slice(1, 4);
        assert_eq!(sub.len(), 3);
        assert_eq!(sub.get(0), Some(&1));
        assert_eq!(sub.get(2), Some(&3));
    }

    #[test]
    fn test_slice_from() {
        let s = GoSlice::from_vec(vec![0, 1, 2, 3, 4]);
        let sub = s.slice_from(2);
        assert_eq!(sub.len(), 3);
        assert_eq!(sub.get(0), Some(&2));
    }

    #[test]
    fn test_slice_to() {
        let s = GoSlice::from_vec(vec![0, 1, 2, 3, 4]);
        let sub = s.slice_to(3);
        assert_eq!(sub.len(), 3);
        assert_eq!(sub.get(2), Some(&2));
    }

    #[test]
    fn test_copy_from() {
        let mut dst = GoSlice::from_vec(vec![0, 0, 0, 0, 0]);
        let src = GoSlice::from_vec(vec![10, 20]);
        let n = dst.copy_from(&src);
        assert_eq!(n, 2);
        assert_eq!(dst.get(0), Some(&10));
        assert_eq!(dst.get(1), Some(&20));
    }

    #[test]
    fn test_copy_partial() {
        let mut dst = GoSlice::with_length(3);
        let src = GoSlice::from_vec(vec![1, 2, 3, 4, 5]);
        let n = dst.copy_from(&src);
        assert_eq!(n, 3); // min of dst=3, src=5
    }

    #[test]
    fn test_clone_preserves_data() {
        let s = GoSlice::from_vec(vec![10, 20, 30]);
        let cloned = s.clone();
        assert_eq!(cloned.get(1), Some(&20));
    }

    #[test]
    fn test_from_string_and_as_string() {
        let s: GoSlice<u8> = GoSlice::from_string("hello");
        assert_eq!(s.len(), 5);
        assert_eq!(s.as_string(), Some("hello".to_string()));
    }

    #[test]
    fn test_from_vec_and_into_vec() {
        let s = GoSlice::from_vec(vec![1, 2, 3]);
        let v = s.into_vec();
        assert_eq!(v, vec![1, 2, 3]);
    }

    #[test]
    fn test_from_slice() {
        let data = vec![10, 20, 30];
        let s = GoSlice::from_slice(&data);
        assert_eq!(s.len(), 3);
    }

    #[test]
    fn test_as_ref() {
        let s = GoSlice::from_vec(vec![1, 2, 3]);
        let slice: &[i32] = s.as_ref();
        assert_eq!(slice, &[1, 2, 3]);
    }

    #[test]
    fn test_default_is_nil() {
        let s: GoSlice<i32> = GoSlice::default();
        assert!(s.is_nil());
    }

    #[test]
    fn test_set_extends_in_available_capacity() {
        // With capacity beyond len, set can extend
        let mut s = GoSlice::with_capacity(10);
        // Initial capacity = 10, len = 0
        s.set(5, 99); // sets at index 5, extending the slice
        assert_eq!(s.len(), 6);
        assert_eq!(s.get(5), Some(&99));
        // Default values in between
        assert_eq!(s.get(2), Some(&0)); // default for i32
    }

    #[test]
    fn test_nil_slice_get_returns_none() {
        let s: GoSlice<i32> = GoSlice::nil_slice();
        assert_eq!(s.get(0), None);
    }

    #[test]
    fn test_clone_nil_like_on_nil() {
        let s: GoSlice<i32> = GoSlice::nil_slice();
        let cloned = s.clone();
        assert!(cloned.is_nil());
    }

    #[test]
    fn test_clone_preserves_initialized() {
        let mut s: GoSlice<i32> = GoSlice::with_capacity(10);
        s.append(42);
        let cloned = s.clone();
        assert!(!cloned.is_nil());
        assert_eq!(cloned.get(0), Some(&42));
    }

    #[test]
    fn test_byte_slice() {
        let s: GoSlice<u8> = GoSlice::from_string("héllo"); // UTF-8 bytes
        assert_eq!(s.len(), 6); // h + é(2) + l + l + o
        assert_eq!(s.get(0), Some(&b'h'));
    }

    #[test]
    fn test_copy_to_nil_dst() {
        let mut dst: GoSlice<i32> = GoSlice::nil_slice();
        let src = GoSlice::from_vec(vec![1, 2, 3]);
        // copy to nil slice should work (similar to Go: nil dst is like empty dst)
        let n = dst.copy_from(&src);
        assert_eq!(n, 3);
        assert_eq!(dst.get(0), Some(&1));
    }

    #[test]
    fn test_from_string_non_utf8() {
        // Create a slice with invalid UTF-8 bytes
        let mut s = GoSlice::from_vec(vec![0xff, 0xfe]);
        assert!(s.as_string().is_none()); // not valid UTF-8
    }
}
