//! Go's standard library built-in functions.
//!
//! Provides `len`, `cap`, `append`, `make_slice`, `make_map`, `copy`, `min`, `max`
//! and helper functions.
//!
//! ## Map helpers (deprecated)
//!
//! The map helper functions in this module are deprecated. New generated code
//! uses `gourd::GoMap<K, V>` with `.get(key)` and `.set(key)` methods instead.
//! These functions are kept for backwards compatibility with legacy output.

use ::std::collections::HashMap;
use ::std::hash::Hash;

use super::super::GoSlice;

/// Returns the length of a slice-like type as i32 (Go `len()`).
pub fn len<T: AsRef<[u8]>>(slice: T) -> i32 {
    slice.as_ref().len() as i32
}

/// Returns the capacity of a vector as i32 (Go `cap()`).
pub fn cap<T: AsRef<[u8]>>(vec: &Vec<T>) -> i32 {
    vec.capacity() as i32
}

/// Appends a value to a slice (Go `append(slice, val)`).
pub fn append<T: Clone>(mut slice: Vec<T>, val: T) -> Vec<T> {
    slice.push(val);
    slice
}

/// Creates a new slice of given length with a default value (Go `make([]T, n)`).
pub fn make_slice<T: Clone + Default>(len: i32, val: &T) -> Vec<T> {
    vec![val.clone(); len as usize]
}

/// Creates a new empty map (Go `make(map[K]V)`).
#[deprecated(since = "0.2.0", note = "Use GoMap::new() instead")]
pub fn make_map<K: Hash + Eq, V>() -> HashMap<K, V> {
    HashMap::new()
}

/// Copies elements from src into dst, returning the number copied (Go `copy(dst, src)`).
pub fn copy<T: Clone>(mut dst: Vec<T>, src: &[T]) -> i32 {
    let n = dst.len().min(src.len());
    dst.clear();
    dst.extend_from_slice(&src[..n]);
    n as i32
}

/// Returns the smaller of two values (Go `min(a, b)`).
pub fn min<T: PartialOrd>(a: T, b: T) -> T {
    if a <= b { a } else { b }
}

/// Returns the larger of two values (Go `max(a, b)`).
pub fn max<T: PartialOrd>(a: T, b: T) -> T {
    if a >= b { a } else { b }
}

/// Go's `copy(dst, src)` — copies elements from src to dst, returns number copied.
/// Deprecated: use GoSlice::copy_from() instead.
pub fn std_copy<T: Clone>(dst: &mut [T], src: &[T]) -> i32 {
    let n = src.len().min(dst.len());
    dst[..n].clone_from_slice(&src[..n]);
    n as i32
}

/// Go's `copy(dst, src)` for both Vec and GoSlice.
/// Accepts either type via From trait (Vec<T> → GoSlice<T>).
pub fn std_copy_slice<T: Clone + 'static>(mut dst: GoSlice<T>, src: &[T]) -> i32 {
    let n = src.len().min(dst.len());
    if n > 0 {
        let tmp = GoSlice::from_slice(src);
        dst.copy_from(&tmp);
    }
    n as i32
}

/// Go's `delete(m, key)` — removes a key from a HashMap, returns the removed value if any.
/// Deprecated: use GoMap::delete() instead.
/// Takes the map by value to avoid mutability issues in Go-style code.
#[deprecated(since = "0.2.0", note = "Use GoMap::delete() instead")]
pub fn std_delete<T: Hash + Eq + Clone, V: Clone, K>(
    map: HashMap<T, V>,
    key: K,
) -> Option<V>
where
    T: PartialEq<K>,
{
    let mut new_map = HashMap::new();
    let mut deleted = None;
    for (k, v) in map {
        if k == key {
            deleted = Some(v);
        } else {
            new_map.insert(k, v);
        }
    }
    deleted
}

/// Go's `append(slice, items...)` — appends items to a slice and returns the new slice.
/// Go `append(slice, items...)` — appends items to a Vec.
/// Deprecated: use GoSlice::append_items() instead.
pub fn std_append<T: Clone>(mut slice: Vec<T>, items: &[T]) -> Vec<T> {
    if !items.is_empty() {
        slice.extend_from_slice(items);
    }
    slice
}

/// Go map read: `m[key]` — returns the value or default for missing keys.
/// Deprecated: use GoMap::get() instead.
#[deprecated(since = "0.2.0", note = "Use GoMap::get() instead")]
pub fn map_get<K: Hash + Eq + Clone, V: Default + Clone>(map: &HashMap<K, V>, key: K) -> V {
    map.get(&key).cloned().unwrap_or_default()
}

/// Map get with borrowed key: for use when iterating over HashMaps where keys are references.
#[deprecated(since = "0.2.0", note = "Use GoMap::get() instead")]
pub fn map_get_ref<K: Hash + Eq + Clone, V: Default + Clone>(map: &HashMap<K, V>, key: &K) -> V {
    map.get(key).cloned().unwrap_or_default()
}

/// Go map write: `m[key] = value` — returns a mutable reference to the map entry.
/// Deprecated: use GoMap::set() instead.
#[deprecated(since = "0.2.0", note = "Use GoMap::set() instead")]
pub fn map_set_mut<'a, K: Hash + Eq + Clone, V: Default + Clone>(
    map: &'a mut HashMap<K, V>,
    key: K,
) -> &'a mut V {
    map.entry(key).or_insert_with(V::default)
}

/// Map set with borrowed key: for use when iterating over HashMaps where keys are references.
#[deprecated(since = "0.2.0", note = "Use GoMap::set() instead")]
pub fn map_set_mut_ref<'a, K: Hash + Eq + Clone, V: Default + Clone>(
    map: &'a mut HashMap<K, V>,
    key: &K,
) -> &'a mut V {
    map.entry(key.clone()).or_insert_with(V::default)
}

/// Go map write with value: `m[key] = val` — inserts a key-value pair.
#[deprecated(since = "0.2.0", note = "Use GoMap::set() then dereference, or insert directly")]
pub fn map_set_val<K: Hash + Eq + Clone, V: Clone>(map: &mut HashMap<K, V>, key: K, val: V) {
    map.insert(key, val);
}

/// Helper for fmt functions to display HashMap<String, i32> values.
#[deprecated(since = "0.2.0", note = "Use GoMap display or format manually")]
pub fn display_map(m: HashMap<String, i32>) -> String {
    let mut result = String::from("{");
    let mut first = true;
    for (k, v) in m.iter() {
        if !first {
            result.push_str(", ");
        }
        result.push_str(k);
        result.push(':');
        result.push_str(&v.to_string());
        first = false;
    }
    result.push('}');
    result
}

/// Display a GoMap<String, i32> as a formatted string.
#[deprecated(since = "0.2.0", note = "Use GoMap display or format manually")]
pub fn display_go_map(m: &crate::GoMap<String, i32>) -> String {
    let mut result = String::from("{");
    let mut first = true;
    for (k, v) in m.inner().iter() {
        if !first {
            result.push_str(", ");
        }
        result.push_str(k);
        result.push(':');
        result.push_str(&v.to_string());
        first = false;
    }
    result.push('}');
    result
}
