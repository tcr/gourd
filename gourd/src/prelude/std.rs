//! Go's standard library built-in functions.
//!
//! Provides `len`, `cap`, `append`, `make_slice`, `make_map`, `copy`, `min`, `max`
//! and helper functions.

use std::collections::HashMap;
use std::hash::Hash;

/// Returns the length of a slice-like type as i32 (Go `len()`).
pub fn len<T: AsRef<[u8]>>(slice: T) -> i32 {
    slice.as_ref().len() as i32
}

/// Returns the capacity of a vector as i32 (Go `cap()`).
pub fn cap<T: AsRef<[u8]>>(vec: &Vec<T>) -> i32 {
    vec.capacity() as i32
}

/// Appends a value to a slice, returning a new slice (Go `append(slice, val)`).
pub fn append<T: Clone + Default>(mut slice: Vec<T>, val: T) -> Vec<T> {
    slice.push(val);
    slice
}

/// Creates a new slice of given length with a default value (Go `make([]T, n)`).
pub fn make_slice<T: Clone + Default>(len: i32, val: T) -> Vec<T> {
    vec![val; len as usize]
}

/// Creates a new empty map (Go `make(map[K]V)`).
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
pub fn std_copy<T: Clone>(dst: &mut [T], src: &[T]) -> i32 {
    let n = src.len().min(dst.len());
    dst[..n].clone_from_slice(&src[..n]);
    n as i32
}

/// Go's `delete(m, key)` — removes a key from a HashMap, returns the removed value if any.
/// Takes the map by value to avoid mutability issues in Go-style code.
pub fn std_delete<T: Hash + Eq + Clone, V: Clone>(
    map: HashMap<T, V>,
    key: T,
) -> Option<V> {
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
pub fn std_append<T: Clone>(mut slice: Vec<T>, items: &[T]) -> Vec<T> {
    slice.extend_from_slice(items);
    slice
}

/// Go map read: `m[key]` — returns the value or default for missing keys.
/// This is the prelude helper for Go-style `count[word]` lookups on HashMaps.
pub fn map_get<K: Hash + Eq + Clone, V: Default + Clone>(map: &HashMap<K, V>, key: K) -> V {
    map.get(&key).cloned().unwrap_or_default()
}

/// Go map write: `m[key] = value` — returns a mutable reference to the map entry,
/// inserting a default value if the key does not exist.
///
/// This is the prelude helper for Go-style `count[word] = count[word] + 1`
/// assignments on HashMaps. The caller dereferences it:
/// `*::gourd::prelude::map_set_mut(count, word) = ...`
pub fn map_set_mut<'a, K: Hash + Eq + Clone, V: Default + Clone>(
    map: &'a mut HashMap<K, V>,
    key: K,
) -> &'a mut V {
    map.entry(key).or_insert_with(V::default)
}

/// Go map write with value: `m[key] = val` — inserts a key-value pair.
/// This handles the full assignment in one call.
pub fn map_set_val<K: Hash + Eq + Clone, V: Clone>(map: &mut HashMap<K, V>, key: K, val: V) {
    map.insert(key, val);
}
