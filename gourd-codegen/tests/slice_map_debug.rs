use gourd_codegen::go_expr;

#[test]
fn test_go_slice_parse() {
    // This test will fail at compile time if parsing fails,
    // so let's check the expanded code with cargo expand
    let v: Vec<i32> = go_expr! { []int{ 1, 2, 3 } };
    println!("Slice: {:?}", v);
    assert_eq!(v, vec![1i32, 2i32, 3i32]);
}

#[test]
fn test_go_slice_empty() {
    let v: Vec<i32> = go_expr! { []int{ } };
    assert!(v.is_empty());
}

#[test]
fn test_go_slice_inferred() {
    let v: Vec<i32> = go_expr! { []{ 10, 20, 30, 40 } };
    assert_eq!(v, vec![10i32, 20i32, 30i32, 40i32]);
}

#[test]
fn test_go_map_string() {
    use std::collections::HashMap;
    let m: HashMap<String, i32> = go_expr! { map[string]int{ "a": 1, "b": 2, "c": 3 } };
    assert_eq!(m.get("a"), Some(&1i32));
    assert_eq!(m.get("b"), Some(&2i32));
    assert_eq!(m.get("c"), Some(&3i32));
    assert_eq!(m.len(), 3);
}

#[test]
fn test_go_map_empty() {
    use std::collections::HashMap;
    let m: HashMap<String, i32> = go_expr! { map[string]int{ } };
    assert!(m.is_empty());
}

#[test]
fn test_go_map_int_keys() {
    use std::collections::HashMap;
    let m: HashMap<i32, String> = go_expr! { map[int]string{ 1: "one", 2: "two" } };
    assert_eq!(m.get(&1i32), Some(&"one".to_string()));
    assert_eq!(m.get(&2i32), Some(&"two".to_string()));
}
