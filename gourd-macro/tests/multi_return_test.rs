use gourd_macro::{go, verify_rust_output};

// Test multi-return
#[verify_rust_output({fn go_divmod(n: i32, d: i32) -> (i32, i32) {
    return (n / d, n % d)
}})]
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
#[verify_rust_output({fn go_triple(a: i32, b: i32) -> (i32, i32, ::gourd::GoString) {
    return (a + b, a * b, ::gourd::GoString::from("pair"))
}})]
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
#[verify_rust_output({fn goMapSize(_a: ::gourd::GoString) -> i32 {
    let m = {
        let mut m = ::gourd::prelude::HashMap::new();
        m.insert(::gourd::GoString::from("a"), 1);
        m.insert(::gourd::GoString::from("b"), 2);
        m.insert(::gourd::GoString::from("c"), 3);
        m
    };
    ;
    return m.len() as i32
}})]
go! {
    func goMapSize(_a string) int {
        m := map[string]int{"a": 1, "b": 2, "c": 3}
        return len(m)
    }
}

#[test]
fn test_map_literal() {
    let result = goMapSize(::gourd::GoString::from("ignored"));
    assert_eq!(result, 3);
}

// Test map length
#[verify_rust_output({fn goEmptyMap() -> i32 {
    let m = {
        let mut m = ::gourd::prelude::HashMap::new();
        m.insert(::gourd::GoString::from("x"), 0);
        m
    };
    ;
    return m.len() as i32
}})]
go! {
    func goEmptyMap() int {
        m := map[string]int{"x": 0}
        return len(m)
    }
}

#[test]
fn test_empty_map() {
    let result = goEmptyMap();
    assert_eq!(result, 1);
}
