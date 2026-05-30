use gourd_codegen::{go, verify_rust_output};

// ── Basic function: no params, return value ─────────────────────────
#[verify_rust_output({
    fn goAdd() -> i32 {
        return 42
    }
})]
go! {
    func goAdd() int {
        return 42
    }
}

// ── Function with mapped parameter type names ───────────────────────
#[verify_rust_output({
    fn goSum(a: i32, b: i32) -> i32 {
        a + b
    }
})]
go! {
    func goSum(a int, b int) int {
        a + b
    }
}

// ── Verify example: compile-time check of transpilation output ───────
#[verify_rust_output({
    fn goAbs(n: i32) -> i32 {
        let mut ret = n; ;
        if n < 0 {
            ret = -ret
        } ;
        return ret
    }
})]
go! {
    func goAbs(n int) int {
        ret := n
        if n < 0 {
            ret = -ret
        }
        return ret
    }
}

// NOTE: This WOULD fail compilation (intentionally commented out):
// Uncomment to see a compile_error showing the expected vs actual mismatch:
// #[gourd_codegen::go_verify({
//     fn go_abs(n: i32) -> i32 {
//         let mut ret = n;
//         if n < 0 {
//             ret = 0;  // Wrong: should be -n
//         }
//         ret
//     }
// })]
// go! {
//     fn go_abs(n int) -> i32 {
//         let mut ret = n;
//         if n < 0 {
//             ret = -n;
//         }
//         ret
//     }
// }


// ── Boolean return ─────────────────────────────────────────────────

#[verify_rust_output({fn is_even(n: i32) -> bool {
        n % 2 == 0
    }})]
go! {
    fn is_even(n: i32) -> bool {
        n % 2 == 0
    }
}

// ── Multiple return values (Go-style `(int, int)` → Rust `(i32, i32)`) ──

#[verify_rust_output({fn go_divmod(n: i32, d: i32) -> (i32, i32) {
        (n / d, n % d)
    }})]
go! {
    fn go_divmod(n int, d int) (int, int) {
        (n / d, n % d)
    }
}

// ── Mixed tuple types: `(int, string)` → `(i32, String)` ──

#[verify_rust_output({fn go_format(n: i32) -> (i32, String) {
        (n, String::from(::std::string::String::from("hello")))
    }})]
go! {
    fn go_format(n int) (int, string) {
        (n, String::from("hello"))
    }
}

// ── Triple multi-return: `(int, int, string)` → `(i32, i32, String)` ──

#[verify_rust_output({fn go_triple(a: i32, b: i32) -> (i32, i32, String) {
        (a + b, a * b, String::from(::std::string::String::from("pair")))
    }})]
go! {
    fn go_triple(a int, b int) (int, int, string) {
        (a + b, a * b, String::from("pair"))
    }
}

// ── String param ────────────────────────────────────────────────────

#[verify_rust_output({fn go_len(s: String) -> i32 {
        s.len() as i32
    }})]
go! {
    fn go_len(s: String) -> i32 {
        s.len() as i32
    }
}

// ── No return ───────────────────────────────────────────────────────

#[verify_rust_output({fn go_incr() -> i32 {
        42
    }})]
go! {
    fn go_incr() -> i32 {
        42
    }
}

#[test]
fn test_fn_return() {
    assert_eq!(goAdd(), 42);
}

#[test]
fn test_fn_with_params() {
    assert_eq!(goSum(10, 20), 30);
}

#[test]
fn test_fn_if_return() {
    assert_eq!(goAbs(-5), 5);
    assert_eq!(goAbs(3), 3);
    assert_eq!(goAbs(0), 0);
}

#[test]
fn test_fn_bool_return() {
    assert!(is_even(4));
    assert!(!is_even(3));
}

#[test]
fn test_fn_multiple_returns() {
    let (q, r) = go_divmod(10, 3);
    assert_eq!(q, 3);
    assert_eq!(r, 1);
}

#[test]
fn test_fn_mixed_tuple_returns() {
    let (n, s) = go_format(42);
    assert_eq!(n, 42);
    assert_eq!(s, "hello");
}

#[test]
fn test_fn_triple_returns() {
    let (s, p, label) = go_triple(3, 4);
    assert_eq!(s, 7);
    assert_eq!(p, 12);
    assert_eq!(label, "pair");
}

#[test]
fn test_fn_string_param() {
    assert_eq!(go_len(String::from("hello")), 5);
}

#[test]
fn test_fn_no_return() {
    let result = go_incr();
    assert_eq!(result, 42);
}

// ── Slice type shorthand ─────────────────────────────────────────────

#[verify_rust_output({fn go_slice_len(a: &[i32]) -> i32 {
        a.len() as i32
    }})]
go! {
    fn go_slice_len(a []int) int {
        len(a)
    }
}

// ── Slice type shorthand (2 params) ──────────────────────────────────

#[verify_rust_output({fn go_slice_subindex(a: &[i32], b: &[i32]) -> i32 {
        a.len() as i32 - b.len() as i32
    }})]
go! {
    fn go_slice_subindex(a, b []int) int {
        len(a) - len(b)
    }
}

#[test]
fn test_slice_type() {
    let data = vec![10, 20, 30];
    assert_eq!(go_slice_len(&data), 3);
    let a = vec![1, 2];
    let b = vec![3];
    assert_eq!(go_slice_subindex(&a, &b), 1);
}

// ── String conversion builtin ────────────────────────────────────────────

#[verify_rust_output({fn go_str(bytes: &[u8]) -> String {
        std::str::from_utf8(&bytes).unwrap_or("").to_string()
    }})]
go! {
    fn go_str(bytes []byte) string {
        string(bytes)
    }
}

#[test]
fn test_string_builtin() {
    let bytes = vec![72, 101, 108, 108, 111];  // "Hello"
    assert_eq!(go_str(&bytes), "Hello");
}

// ── Go-style parameter shorthand: group multiple params with shared type ────

#[verify_rust_output({fn go_shorthand(a: i32, b: i32, c: i32) -> i32 {
        a + b + c
    }})]
go! {
    fn go_shorthand(a, b, c int) int {
        a + b + c
    }
}

#[test]
fn test_param_grouping() {
    assert_eq!(go_shorthand(1, 2, 3), 6);
}


#[verify_rust_output({fn hello() -> String {
        String::from(::std::string::String::from("hello"))
    }})]
go! {
    fn hello() string {
        String::from("hello")
    }
}

#[test]
fn test_error_signature_check() {
    let _ = hello();
}

// ── Slice/map literals inside go! function bodies ────────────────────


#[verify_rust_output({fn go_slice_literal() -> Vec<i32> {
        vec![1, 2, 3]
    }})]
go! {
    fn go_slice_literal() Vec<i32> {
        []int{ 1, 2, 3 }
    }
}

#[test]
fn test_slice_literal_in_body() {
    let v = go_slice_literal();
    assert_eq!(v, vec![1i32, 2i32, 3i32]);
}


#[verify_rust_output({fn go_slice_literal_empty() -> Vec<i32> {
        vec![]
    }})]
go! {
    fn go_slice_literal_empty() Vec<i32> {
        []int{}
    }
}

#[test]
fn test_slice_literal_empty_in_body() {
    let v = go_slice_literal_empty();
    assert!(v.is_empty());
}


#[verify_rust_output({fn go_slice_literal_type_inferred() -> Vec<i32> {
        vec![2, 3, 4]
    }})]
go! {
    fn go_slice_literal_type_inferred() Vec<i32> {
        []{ 2, 3, 4 }
    }
}

#[test]
fn test_slice_literal_type_inferred_in_body() {
    let v = go_slice_literal_type_inferred();
    assert_eq!(v, vec![2i32, 3i32, 4i32]);
}

// NOTE: this hangs during build when uncommented:
//
// go! {
//     fn go_map_literal(a string) int {
//         let m = map[string]int{ "a": 1, "b": 2, "c": 3 };
//         *m.get(a).unwrap()
//     }
// }
//
// #[test]
// fn test_map_literal_in_body() {
//     use std::collections::HashMap;
//     let result = go_map_literal(String::from("b"));
//     assert_eq!(result, 2i32);
//     let result = go_map_literal(String::from("a"));
//     assert_eq!(result, 1i32);
// }


#[verify_rust_output({fn go_map_literal_empty() -> bool {
        let m = std::collections::HashMap::<String, i32>::default(); ;
        m.is_empty()
    }})]
go! {
    fn go_map_literal_empty() bool {
        let m = map[string]int{ };
        m.is_empty()
    }
}

#[test]
fn test_map_literal_empty_in_body() {
    assert!(go_map_literal_empty());
}


#[verify_rust_output({fn go_int_map() -> String {
        let m = {
            let mut m = std::collections::HashMap::<i32, String>::new();
            m.insert(1, ::std::string::String::from("one"));
            m.insert(2, ::std::string::String::from("two"));
            m
        } ; ;
        m.get(&2).unwrap().clone()
    }})]
go! {
    fn go_int_map() String {
        let m = map[int]string{ 1: "one", 2: "two" };
        m.get(2).unwrap().clone()
    }
}

#[test]
fn test_int_map_in_body() {
    let result = go_int_map();
    assert_eq!(result, "two".to_string());
}
