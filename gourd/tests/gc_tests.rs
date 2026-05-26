use gourd::GoGc;

#[test]
fn test_new_creates_single_reference() {
    let gc = GoGc::new(42i32);
    assert_eq!(*gc, 42);
    assert_eq!(gc.strong_count(), 1);
}

#[test]
fn test_clone_increments_reference_count() {
    let gc = GoGc::new("hello".to_string());
    assert_eq!(gc.strong_count(), 1);
    let clone = GoGc::clone(&gc);
    assert_eq!(gc.strong_count(), 2);
    assert_eq!(*clone, "hello");
}

#[test]
fn test_deref_access_fields() {
    struct Point { x: i32, y: i32 }
    let gc = GoGc::new(Point { x: 10, y: 20 });
    assert_eq!(gc.x, 10);
    assert_eq!(gc.y, 20);
}

#[test]
fn test_into_inner_preserves_reference_count() {
    let gc = GoGc::new(100i32);
    assert_eq!(gc.strong_count(), 1);
    let arc = GoGc::into_inner(gc);
    assert_eq!(std::sync::Arc::strong_count(&arc), 1);
}

#[test]
fn test_try_unwrap_succeeds_with_single_reference() {
    let gc = GoGc::new("unique".to_string());
    match GoGc::try_unwrap(gc) {
        Ok(inner) => assert_eq!(inner, "unique"),
        Err(_) => panic!("try_unwrap should succeed when refcount == 1"),
    }
}

#[test]
fn test_try_unwrap_fails_with_multiple_references() {
    let gc = GoGc::new(7i32);
    let _clone = GoGc::clone(&gc);
    assert_eq!(gc.strong_count(), 2);
    match GoGc::try_unwrap(gc) {
        Ok(_) => panic!("try_unwrap should fail when refcount > 1"),
        Err(back) => assert_eq!(back.strong_count(), 2),
    }
}

#[test]
fn test_eq_and_partial_cmp() {
    let a = GoGc::new(42i32);
    let b = GoGc::new(42i32);
    let c = GoGc::new(99i32);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn test_display() {
    let gc = GoGc::new("world".to_string());
    assert_eq!(format!("{}", gc), "world");
}
