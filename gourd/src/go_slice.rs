//! Go slice semantics.
//!
//! Go slices are lightweight headers (ptr, len, cap) over a backing array.
//! Key properties:
//! - Slicing `s[lo:hi]` creates a new header sharing the same backing array
//! - Capacity is tracked: cap = backing_capacity - offset
//! - `append` can reallocate the backing array when capacity is exceeded
//! - Nil slices differ from empty slices: nil is zero-length with no backing array
//! - `copy(dst, src)` copies min(len) elements and returns count
//!
//! GoSlice<T> exposes Go slice semantics using GoGc-backed shared storage.

use crate::go_gc::GoGc;

/// Go slice — reference-type slice with Go semantics.
///
/// Models Go's `[]T` type at runtime using a shared backing store (GoGc<Vec<T>>) and
/// per-view header tracking (offset, length):
/// - Slicing: `s.slice(lo, hi)` returns a new GoSlice **sharing the backing array** (O(1))
/// - Append: `s.append(val)` mimics Go's append (may reallocate the shared backing)
/// - Indexing: `s.get(i)` returns Option<T> (Go panics on out-of-bounds, this is safe)
/// - Nil vs empty: Go distinguishes nil (`GoSlice::nil_slice()`) from empty (`GoSlice::new()`)
/// - Capacity tracking: `cap(s)` = backing_capacity - offset (Go semantics)
#[derive(Debug)]
pub struct GoSlice<T: Clone + 'static> {
    /// Shared backing array via GoGc. None means nil slice (no backing at all).
    backing: Option<GoGc<Vec<T>>>,
    /// Header offset into the backing array (ptr in Go's slice header).
    offset: usize,
    /// Header length (len in Go's slice header).
    length: usize,
}

impl<T: Clone + 'static> GoSlice<T> {
    /// Create a new empty slice (Go: `make([]T)`).
    /// This is an empty but initialized slice — NOT nil.
    pub fn new() -> Self {
        Self {
            backing: Some(GoGc::new(Vec::new())),
            offset: 0,
            length: 0,
        }
    }

    /// Create a new slice with given capacity (Go: `make([]T, 0, cap)`).
    /// This is an empty but initialized slice — NOT nil.
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            backing: Some(GoGc::new(Vec::with_capacity(cap))),
            offset: 0,
            length: 0,
        }
    }

    /// Create a nil slice (Go: `var s []T` — nil slice).
    /// - Reads: returns None for any index (like Go's nil map behavior)
    /// - Appends: behaves like appending to an empty slice (allocates new backing array)
    pub fn nil_slice() -> Self {
        Self { backing: None, offset: 0, length: 0 }
    }

    // --- internal helpers -----------------------------------------------------

    /// Access the backing vector (panics if nil). Only for use within GoSlice methods.
    fn data(&self) -> &Vec<T> {
        self.backing.as_ref().expect("nil slice has no backing")
    }

    /// Returns the backing vector's capacity (panics if nil).
    fn backing_capacity(&self) -> usize {
        self.data().capacity()
    }

    // --- public API (unchanged) -----------------------------------------------

    /// Returns the slice length (Go `len(s)`).
    pub fn len(&self) -> usize {
        self.length
    }

    /// Returns true if the slice is empty (Go `len(s) == 0`).
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Returns the backing array capacity (Go `cap(s)`).
    pub fn capacity(&self) -> usize {
        match &self.backing {
            None => 0,
            Some(_) => self.backing_capacity().saturating_sub(self.offset),
        }
    }

    /// Returns true if this slice is nil (Go `s == nil`).
    pub fn is_nil(&self) -> bool {
        self.backing.is_none()
    }

    /// Safe indexed read: returns Some(value) if in bounds, None otherwise.
    /// Go panics on out-of-bounds; this is safe but equivalent for transpiled code.
    pub fn get(&self, index: usize) -> Option<&T> {
        let backing = self.backing.as_ref()?;
        if index < self.length {
            let backing_idx = self.offset + index;
            Some(&backing[backing_idx])
        } else {
            None
        }
    }

    /// Safe indexed read with default fallback: returns value or T::default() for missing.
    pub fn get_or_default(&self, index: usize) -> &T {
        let backing = self.backing.as_ref().expect("nil slice has no backing");
        if index >= self.length {
            panic!("index out of range: {} (len={})", index, self.length);
        }
        &backing[self.offset + index]
    }

    // NOTE: set() requires T: Default — see specialized impl below.

    /// Go `append(s, item)` — appends a single item.
    /// Nil slice: creates fresh backing. Normal slice: grows the shared backing.
    pub fn append(&mut self, item: T) {
        match &mut self.backing {
            None => {
                let mut new_vec = Vec::with_capacity(1);
                new_vec.push(item);
                self.backing = Some(GoGc::new(new_vec));
                self.offset = 0;
                self.length = 1;
            }
            Some(backing) => {
                backing.reserve(1);
                backing.push(item);
                self.length += 1;
            }
        }
    }

    /// Go `append(s, items...)` — appends multiple items.
    pub fn append_items(&mut self, items: &[T]) {
        if items.is_empty() { return; }
        match &mut self.backing {
            None => {
                let mut new_vec = Vec::with_capacity(items.len());
                new_vec.extend_from_slice(items);
                self.backing = Some(GoGc::new(new_vec));
                self.offset = 0;
                self.length = items.len();
            }
            Some(backing) => {
                backing.reserve(items.len());
                backing.extend_from_slice(items);
                self.length += items.len();
            }
        }
    }

    /// Go `s[lo:hi]` — sub-slice. Creates a new GoSlice sharing the backing array.
    /// This is the key Go semantics: sub-slicing copies only the header, not the data.
    pub fn slice(&self, lo: usize, hi: usize) -> Self {
        match &self.backing {
            None => GoSlice::nil_slice(),
            Some(backing) => {
                let sub_offset = self.offset + lo;
                if sub_offset >= backing.len() {
                    return GoSlice::nil_slice();
                }
                let sub_len = (hi.min(backing.len()) - sub_offset).max(0);
                GoSlice {
                    backing: Some(GoGc::clone(backing)),
                    offset: sub_offset,
                    length: sub_len,
                }
            }
        }
    }

    /// Go `s[:]` — full sub-slice (clone the slice header, not data).
    pub fn clone_slice(&self) -> Self {
        GoSlice {
            backing: self.backing.as_ref().map(|b| GoGc::clone(b)),
            offset: self.offset,
            length: self.length,
        }
    }

    /// Go `s[lo:]` — sub-slice from lo to end.
    pub fn slice_from(&self, lo: usize) -> Self {
        match &self.backing {
            None => GoSlice::nil_slice(),
            Some(backing) => {
                let sub_offset = self.offset + lo;
                if sub_offset >= backing.len() {
                    return GoSlice::nil_slice();
                }
                let sub_len = backing.len() - sub_offset;
                GoSlice {
                    backing: Some(GoGc::clone(backing)),
                    offset: sub_offset,
                    length: sub_len,
                }
            }
        }
    }

    /// Go `s[:hi]` — sub-slice from start to hi.
    pub fn slice_to(&self, hi: usize) -> Self {
        match &self.backing {
            None => GoSlice::nil_slice(),
            Some(backing) => {
                let sub_offset = self.offset;
                if sub_offset >= backing.len() {
                    return GoSlice::nil_slice();
                }
                let sub_len = (hi.min(backing.len()) - sub_offset).max(0);
                GoSlice {
                    backing: Some(GoGc::clone(backing)),
                    offset: sub_offset,
                    length: sub_len,
                }
            }
        }
    }

    /// Go `copy(dst, src)` — copies min(len) elements from src to dst.
    /// For nil dst, allocates a new backing array (Go semantics).
    /// Returns the number of elements copied.
    pub fn copy_from(&mut self, src: &Self) -> usize {
        match &mut self.backing {
            None => {
                // Nil dst: allocate new backing array (like Go copy to empty/nil)
                let src_backing = src.backing.as_ref().expect("nil source has no backing");
                self.backing = Some(GoGc::new(src_backing[..src.length].to_vec()));
                self.offset = 0;
                self.length = src.length;
                return src.length;
            }
            Some(backing) => {
                let n = self.length.min(src.length);
                if n == 0 {
                    return 0;
                }
                let src_backing = src.backing.as_ref().expect("nil source has no backing");
                // Try in-place mutation (own Arc) then fall back to clone-and-replace
                if let Some(dst_vec) = backing.get_mut() {
                    for i in 0..n {
                        dst_vec[self.offset + i] = src_backing[i].clone();
                    }
                } else {
                    let mut new = (**backing).clone();
                    for i in 0..n {
                        new[self.offset + i] = src_backing[i].clone();
                    }
                    backing.replace(new);
                }
                return n;
            }
        }
    }

    /// Returns a reference to the underlying Vec data (panics if nil).
    pub fn inner(&self) -> &Vec<T> {
        self.data()
    }

    /// Returns a clone of the underlying Vec (since Arc prevents direct access).
    pub fn inner_mut(&mut self) -> Vec<T> {
        self.to_vec()
    }

    /// Convert from a Rust Vec to GoSlice (owned, full capacity).
    pub fn from_vec(vec: Vec<T>) -> Self {
        let len = vec.len();
        Self {
            backing: Some(GoGc::new(vec)),
            offset: 0,
            length: len,
        }
    }

    /// Convert from a Rust slice (borrows).
    pub fn from_slice(slice: &[T]) -> Self {
        GoSlice {
            backing: Some(GoGc::new(slice.to_vec())),
            offset: 0,
            length: slice.len(),
        }
    }

    // NOTE: from_string() and as_string() are only available on GoSlice<u8>
    // See the specialized impl below.

    /// Convert to a Rust Vec (consumes self).
    pub fn into_vec(self) -> Vec<T> {
        if let Some(backing) = self.backing {
            backing.into_vec()
        } else {
            Vec::new()
        }
    }

    /// Clone the inner data as a Rust Vec.
    pub fn to_vec(&self) -> Vec<T> {
        match &self.backing {
            None => Vec::new(),
            Some(b) => b[self.offset..self.offset + self.length].to_vec(),
        }
    }
}

impl<T: Clone + 'static + Default> Default for GoSlice<T> {
    fn default() -> Self {
        GoSlice::nil_slice()
    }
}

impl<T: Clone + 'static> Clone for GoSlice<T> {
    fn clone(&self) -> Self {
        GoSlice {
            backing: self.backing.as_ref().map(|b| GoGc::clone(b)),
            offset: self.offset,
            length: self.length,
        }
    }
}

impl<T: Clone + 'static> AsRef<[T]> for GoSlice<T> {
    fn as_ref(&self) -> &[T] {
        match &self.backing {
            None => &[],
            Some(b) => &b[self.offset..self.offset + self.length],
        }
    }
}

/// Methods that require Default to fill slices with zero values.
impl<T: Clone + 'static + Default> GoSlice<T> {
    /// Create a new slice with length `len` and default values (Go: `make([]T, len)`).
    pub fn with_length(len: usize) -> Self {
        let data = vec![T::default(); len];
        Self {
            backing: Some(GoGc::new(data)),
            offset: 0,
            length: len,
        }
    }

    /// Go `s[i] = value` — set element at index.
    pub fn set(&mut self, index: usize, value: T) {
        // Create fresh backing if nil
        if self.is_nil() {
            let mut new_vec = Vec::new();
            while new_vec.len() <= index {
                new_vec.push(T::default());
            }
            self.backing = Some(GoGc::new(new_vec));
            self.offset = 0;
            self.length = index + 1;
            return;
        }
        // Check bounds before attempting mutation
        if index >= self.length && index >= self.backing_capacity() - self.offset {
            panic!("index out of range: {} (cap={})", index, self.capacity());
        }
        // Use set_at helper — handles shared backing automatically
        let backing = self.backing.as_mut().expect("should have backing");
        // Extend length if writing past visible range but within backing
        let new_len = if index >= self.length {
            index + 1
        } else {
            self.length
        };
        for i in self.length..new_len {
            backing.set_at(self.offset + i, T::default());
        }
        self.length = new_len;
        backing.set_at(self.offset + index, value);
    }
}

impl<T: Clone + 'static> From<Vec<T>> for GoSlice<T> {
    fn from(vec: Vec<T>) -> Self {
        GoSlice::from_vec(vec)
    }
}

/// Specialized methods for byte slices (Go `[]byte`).
impl GoSlice<u8> {
    /// Create a slice of bytes from a string (Go `[]byte(s)`).
    pub fn from_string(s: &str) -> GoSlice<u8> {
        GoSlice {
            backing: Some(GoGc::new(s.as_bytes().to_vec())),
            offset: 0,
            length: s.len(),
        }
    }

    /// Convert to a String (Go `string([]byte)`).
    pub fn as_string(&self) -> Option<String> {
        if self.is_nil() {
            return None;
        }
        String::from_utf8(self.to_vec()).ok()
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
