use gourd_codegen::go;

// Test multi-return
go! {
    func go_divmod(n int, d int) (int, int) {
        return n / d, n % d
    }
}

#[test]
fn test_multi_return() {
    let (q, r) = go_divmod(10, 3);
    assert_eq!(q, 3);
    assert_eq!(r, 1);
}

// Test triple multi-return
go! {
    func go_triple(a int, b int) (int, int, string) {
        return a + b, a * b, "pair"
    }
}

#[test]
fn test_triple_return() {
    let (s, p, label) = go_triple(3, 4);
    assert_eq!(s, 7);
    assert_eq!(p, 12);
    assert_eq!(label, "pair");
}

// Test string-keyed map literal
go! {
    func go_map_size(a string) int {
        m := map[string]int{"a": 1, "b": 2, "c": 3}
        return len(m)
    }
}

#[test]
fn test_map_literal() {
    let result = go_map_size(String::from("ignored"));
    assert_eq!(result, 3);
}

// Test map length
use std::collections::HashMap;

go! {
    func go_empty_map() int {
        m := map[string]int{"x": 0}
        return len(m)
    }
}

#[test]
fn test_empty_map() {
    let result = go_empty_map();
    assert_eq!(result, 1);
}
