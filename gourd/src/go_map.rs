//! Go map semantics.
//!
//! Go maps are reference types with these key properties:
//! - Nil maps are valid: reads return zero values, writes panic
//! - `m[key]` returns V::default() for missing keys (not an error)
//! - `m[key] = value` inserts a new entry (creating one with default if missing)
//! - `delete(m, key)` removes an entry in-place
//! - Maps are reference types: copy shares the backing store

use std::collections::HashMap;
use std::hash::Hash;

/// Go map — reference-type map with Go semantics.
///
/// Models Go's `map[K]V` type at runtime:
/// - Nil maps: safe reads return zero values, writes panic
/// - Index read with default: `m.get(key)` → V::default() for missing keys
/// - Index write with default: `m.set(key)` → creates entry with default if missing
/// - Delete: `m.delete(key)` → removes in-place, returns value if present
///
/// Unlike Rust's HashMap which is owned by the variable, Go maps are reference
/// types — copying a map variable just copies the reference.
#[derive(Debug)]
pub struct GoMap<K: Hash + Eq, V: Default + Clone> {
    map: HashMap<K, V>,
    initialized: bool, // Tracks whether this is a "nil" map (Go: nil map)
}

impl<K: Hash + Eq, V: Default + Clone> GoMap<K, V> {
    /// Create a nil-like map (Go: nil map).
    /// - Reads: returns V::default() for any key (valid, no panic)
    /// - Writes: panic (matching Go behavior — writing to nil map panics)
    pub fn nil_map() -> Self {
        Self {
            map: HashMap::new(),
            initialized: false,
        }
    }

    /// Create an initialized empty map (Go: `make(map[K]V)`).
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            initialized: true,
        }
    }

    /// Create an initialized map with a capacity hint (Go: `make(map[K]V, hint)`).
    /// The hint is an optimization — it pre-allocates space for approximately `hint` entries.
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            map: HashMap::with_capacity(cap),
            initialized: true,
        }
    }

    /// Go `m[key]` — returns value or V::default() for missing/nil keys.
    /// This is the prelude helper for Go-style `count[word]` lookups on maps.
    pub fn get(&self, key: &K) -> V {
        if !self.initialized {
            return V::default();
        }
        self.map.get(key).cloned().unwrap_or_default()
    }

    /// Go `m[key]` — returns (value, ok) semantics.
    /// Returns Some(value) if key exists and map is initialized, None otherwise.
    pub fn get_ok(&self, key: &K) -> Option<V> {
        if !self.initialized {
            return None;
        }
        self.map.get(key).cloned()
    }

    /// Go `m[key] = value` — insert or create entry with default.
    /// If nil map, panics (matching Go behavior).
    /// Returns a mutable reference to the entry.
    pub fn set(&mut self, key: K) -> &mut V {
        assert!(self.initialized, "cannot write to nil map");
        self.map.entry(key).or_insert_with(V::default)
    }

    /// Insert a key-value pair (HashMap-style API for compatibility).
    /// Returns the old value at key if it existed, None otherwise.
    pub fn insert(&mut self, key: K, val: V) -> Option<V> {
        assert!(self.initialized, "cannot write to nil map");
        self.map.insert(key, val)
    }

    /// Go `delete(m, key)` — removes entry, returns value if present.
    /// If nil map, does nothing and returns None (matching Go behavior).
    pub fn delete(&mut self, key: K) -> Option<V> {
        if !self.initialized {
            return None;
        }
        self.map.remove(&key)
    }

    /// Returns the map length (Go `len(m)`).
    pub fn len(&self) -> i32 {
        self.map.len() as i32
    }

    /// Returns true if the map is empty or nil (Go `m == nil || len(m) == 0`).
    pub fn is_empty(&self) -> bool {
        !self.initialized || self.map.is_empty()
    }

    /// Returns true if this map is nil (not initialized).
    pub fn is_nil(&self) -> bool {
        !self.initialized
    }

    /// Returns a reference to the underlying HashMap.
    pub fn inner(&self) -> &HashMap<K, V> {
        &self.map
    }

    /// Returns a mutable reference to the underlying HashMap.
    pub fn inner_mut(&mut self) -> &mut HashMap<K, V> {
        &mut self.map
    }

    /// Go `for k, v := range map` — iterator over key-value pairs.
    /// Returns an empty iterator for nil maps (matching Go behavior).
    pub fn iter(&self) -> Box<dyn Iterator<Item = (&K, &V)> + '_> {
        if !self.initialized {
            Box::new(std::iter::empty())
        } else {
            Box::new(self.map.iter())
        }
    }

    /// Go `for k := range map` — iterator over keys only.
    pub fn keys(&self) -> Box<dyn Iterator<Item = &K> + '_> {
        if !self.initialized {
            Box::new(std::iter::empty())
        } else {
            Box::new(self.map.keys())
        }
    }

    /// Clone the map but keep it as nil if the original was nil.
    /// Go maps have reference semantics — cloning just copies the reference.
    pub fn clone_nil_like(&self) -> Self
    where K: Clone, V: Clone
    {
        if !self.initialized {
            GoMap::nil_map()
        } else {
            Self {
                map: self.map.clone(),
                initialized: true,
            }
        }
    }

    /// Convert a Go nil map to an initialized empty map.
    pub fn force_init(&mut self) {
        if !self.initialized {
            self.map = HashMap::new();
            self.initialized = true;
        }
    }
}

impl<K: Hash + Eq + Clone, V: Default + Clone> Clone for GoMap<K, V> {
    fn clone(&self) -> Self {
        // Go maps are reference types — shallow clone matches Go semantics
        // (copying a map variable doesn't deep-copy the backing store)
        Self {
            map: self.map.clone(),
            initialized: self.initialized,
        }
    }
}

impl<K: Hash + Eq, V: Default + Clone> Default for GoMap<K, V> {
    fn default() -> Self {
        GoMap::nil_map()
    }
}

impl<K: Hash + Eq + std::fmt::Display, V: Default + Clone + std::fmt::Display> std::fmt::Display for GoMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_nil() {
            return write!(f, "nil map");
        }
        let inner = self.inner();
        write!(f, "{{")?;
        let mut first = true;
        for (k, v) in inner {
            if !first { write!(f, ", ")?; }
            write!(f, "{}: {}", k, v)?;
            first = false;
        }
        write!(f, "}}")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nil_map_reads_return_default() {
        let m: GoMap<String, i32> = GoMap::nil_map();
        assert_eq!(m.get(&"key".to_string()), 0); // default for i32
    }

    #[test]
    fn test_nil_map_get_ok_returns_none() {
        let m: GoMap<String, i32> = GoMap::nil_map();
        assert_eq!(m.get_ok(&"key".to_string()), None);
    }

    #[test]
    fn test_nil_map_write_panics() {
        let mut m: GoMap<String, i32> = GoMap::nil_map();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            m.set("key".to_string());
        }))
        .is_err());
    }

    #[test]
    fn test_new_initialized_map() {
        let mut m: GoMap<String, i32> = GoMap::new();
        *m.set("key".to_string()) = 42;
        assert_eq!(m.get(&"key".to_string()), 42);
    }

    #[test]
    fn test_map_set_returns_mut_ref() {
        let mut m: GoMap<String, i32> = GoMap::new();
        *m.set("a".to_string()) = 10;
        *m.set("a".to_string()) += 5;
        assert_eq!(m.get(&"a".to_string()), 15);
    }

    #[test]
    fn test_map_delete() {
        let mut m: GoMap<String, i32> = GoMap::new();
        *m.set("key".to_string()) = 42;
        let deleted = m.delete("key".to_string());
        assert_eq!(deleted, Some(42));
        assert_eq!(m.get(&"key".to_string()), 0); // now missing, returns default
    }

    #[test]
    fn test_nil_map_delete_does_nothing() {
        let mut m: GoMap<String, i32> = GoMap::nil_map();
        let deleted = m.delete("key".to_string());
        assert_eq!(deleted, None);
    }

    #[test]
    fn test_map_len() {
        let mut m: GoMap<String, i32> = GoMap::new();
        assert_eq!(m.len(), 0);
        *m.set("a".to_string()) = 1;
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn test_map_is_empty() {
        let mut m: GoMap<String, i32> = GoMap::new();
        assert!(m.is_empty());
        *m.set("a".to_string()) = 1;
        assert!(!m.is_empty());
    }

    #[test]
    fn test_map_is_nil() {
        let m: GoMap<String, i32> = GoMap::nil_map();
        assert!(m.is_nil());
        let m2: GoMap<String, i32> = GoMap::new();
        assert!(!m2.is_nil());
    }

    #[test]
    fn test_force_init() {
        let mut m: GoMap<String, i32> = GoMap::nil_map();
        assert!(m.is_nil());
        m.force_init();
        assert!(!m.is_nil());
        *m.set("key".to_string()) = 1;
        assert_eq!(m.get(&"key".to_string()), 1);
    }

    #[test]
    fn test_clone_preserves_nil() {
        let m: GoMap<String, i32> = GoMap::nil_map();
        let cloned = m.clone();
        assert!(cloned.is_nil());
    }

    #[test]
    fn test_clone_preserves_initialized() {
        let mut m: GoMap<String, i32> = GoMap::new();
        *m.set("key".to_string()) = 42;
        let cloned = m.clone();
        assert!(!cloned.is_nil());
        assert_eq!(cloned.get(&"key".to_string()), 42);
    }

    #[test]
    fn test_clone_nil_like_on_nil() {
        let m: GoMap<String, i32> = GoMap::nil_map();
        let cloned = m.clone_nil_like();
        assert!(cloned.is_nil());
    }

    #[test]
    fn test_clone_nil_like_on_initialized() {
        let mut m: GoMap<String, i32> = GoMap::new();
        *m.set("key".to_string()) = 42;
        let cloned = m.clone_nil_like();
        assert!(!cloned.is_nil());
        assert_eq!(cloned.get(&"key".to_string()), 42);
    }

    #[test]
    fn test_default_is_nil() {
        let m: GoMap<String, i32> = GoMap::default();
        assert!(m.is_nil());
    }

    #[test]
    fn test_get_ok_returns_some_for_existing_key() {
        let mut m: GoMap<String, i32> = GoMap::new();
        *m.set("key".to_string()) = 42;
        assert_eq!(m.get_ok(&"key".to_string()), Some(42));
    }

    #[test]
    fn test_get_ok_returns_none_for_missing_key() {
        let mut m: GoMap<String, i32> = GoMap::new();
        assert_eq!(m.get_ok(&"missing".to_string()), None);
    }

    #[test]
    fn test_inner_access() {
        let m: GoMap<String, i32> = GoMap::nil_map();
        assert_eq!(m.inner().len(), 0);

        let mut m2: GoMap<String, i32> = GoMap::new();
        *m2.set("key".to_string()) = 1;
        assert_eq!(m2.inner().get(&"key".to_string()), Some(&1));
    }

    #[test]
    fn test_int_key_map() {
        let mut m: GoMap<i32, String> = GoMap::new();
        *m.set(42) = "answer".to_string();
        assert_eq!(m.get(&42), "answer".to_string());
    }
}
