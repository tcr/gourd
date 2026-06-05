// Test that the new prelude map helpers work correctly.
use std::collections::HashMap;
use gourd::prelude::{map_get, map_set_mut, map_set_val};

#[test]
fn test_map_get_returns_value_or_default() {
    let mut map: HashMap<String, i32> = HashMap::new();
    map.insert("a".to_string(), 42);
    map.insert("b".to_string(), 7);

    assert_eq!(map_get(&map, "a".to_string()), 42);
    assert_eq!(map_get(&map, "b".to_string()), 7);
    assert_eq!(map_get(&map, "c".to_string()), 0); // default for missing key
}

#[test]
fn test_map_set_mut_returns_mut_ref_with_or_default() {
    let mut map: HashMap<String, i32> = HashMap::new();
    map.insert("a".to_string(), 42);

    // Or-inserts default when key missing.
    assert_eq!(*map_set_mut(&mut map, "b".to_string()), 0);
    assert_eq!(map.get("b"), Some(&0));

    // Existing key returns reference to stored value.
    assert_eq!(*map_set_mut(&mut map, "a".to_string()), 42);

    // Can mutate the returned reference.
    *map_set_mut(&mut map, "a".to_string()) += 10;
    assert_eq!(map.get("a"), Some(&52));
}

#[test]
fn test_map_set_val_inserts_pair() {
    let mut map: HashMap<String, i32> = HashMap::new();

    map_set_val(&mut map, "a".to_string(), 42);
    map_set_val(&mut map, "b".to_string(), 7);

    assert_eq!(map.get("a"), Some(&42));
    assert_eq!(map.get("b"), Some(&7));
    assert_eq!(map.len(), 2);
}
