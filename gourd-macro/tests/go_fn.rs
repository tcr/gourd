use gourd_macro::{go, verify_rust_output};

// ── Basic function: no params, return value ─────────────────────────
#[verify_rust_output({
    fn goAdd() -> i32 {
        return 42
    }
})]
go! {
    func goAdd() int {
        return 42;
    }
}

// ── Function with mapped parameter type names ───────────────────────
#[verify_rust_output({
    fn goSum(a: i32, b: i32) -> i32 {
        return a + b
    }
})]
go! {
    func goSum(a int, b int) int {
        return a + b;
    }
}

// ── Verify example: compile-time check of transpilation output ───────
// ── Control flow: if statements ──────────────────────────────────────

#[verify_rust_output({fn goAbs(n: i32) -> i32 {
        let mut ret = n ; ; if n < 0 { ret = - ret } ; return ret
    }})]
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
// #[gourd_macro::go_verify({
//     fn goAbs(n: i32) -> i32 {
//         let mut ret = n;
//         if n < 0 {
//             ret = 0;  // Wrong: should be -n
//         }
//         ret
//     }
// })]
// go! {
//     fn goAbs(n int) -> i32 {
//         let mut ret = n;
//         if n < 0 {
//             ret = -n;
//         }
//         ret
//     }
// }


// ── Boolean return ─────────────────────────────────────────────────

#[verify_rust_output({fn isEven(n: i32) -> bool {
        return n % 2 == 0
    }})]
go! {
    func isEven(n int) bool {
        return n % 2 == 0
    }
}

// ── Multiple return values (Go-style `(int, int)` → Rust `(i32, i32)`) ──

// NOTE: multi-return not yet implemented in transpiler
// #[verify_rust_output({fn go_divmod(n: i32, d: i32) -> (i32, i32) {
//         return n / d, n % d
//     }})]
// go! {
//     func goDivmod(n int, d int) (int, int) {
//         return n / d, n % d
//     }
// }

// ── Mixed tuple types: `(int, string)` → `(i32, String)` ──

// #[verify_rust_output({fn go_format(n: i32) -> (i32, String) {
//         return n, "hello"
//     }})]
// go! {
//     func goFormat(n int) (int, string) {
//         return n, "hello"
//     }
// }

// ── Triple multi-return: `(int, int, string)` → `(i32, i32, String)` ──

// #[verify_rust_output({fn go_triple(a: i32, b: i32) -> (i32, i32, String) {
//         return a + b, a * b, "pair"
//     }})]
// go! {
//     func goTriple(a int, b int) (int, int, string) {
//         return a + b, a * b, "pair"
//     }
// }

// NOTE: type conversion (int()) not yet supported in transpiler
// // ── String param ────────────────────────────────────────────────────

// #[verify_rust_output({fn goLen(s: String) -> i32 {
//         return int(s.len() as i32)
//     }})]
// go! {
//     func goLen(s string) int {
//         return int(len(s))
//     }
// }

// ── No return ───────────────────────────────────────────────────────

#[verify_rust_output({fn goIncr() -> i32 {
        return 42
    }})]
go! {
    func goIncr() int {
        return 42
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
    assert!(isEven(4));
    assert!(!isEven(3));
}

// NOTE: multi-return not yet implemented
// #[test]
// fn test_fn_multiple_returns() {
//     let (q, r) = go_divmod(10, 3);
//     assert_eq!(q, 3);
//     assert_eq!(r, 1);
// }

// #[test]
// fn test_fn_mixed_tuple_returns() {
//     let (n, s) = go_format(42);
//     assert_eq!(n, 42);
//     assert_eq!(s, "hello");
// }

// #[test]
// fn test_fn_triple_returns() {
//     let (s, p, label) = go_triple(3, 4);
//     assert_eq!(s, 7);
//     assert_eq!(p, 12);
//     assert_eq!(label, "pair");
// }

// #[test]
// fn test_fn_string_param() {
//     assert_eq!(goLen(String::from("hello")), 5);
// }

#[test]
fn test_fn_no_return() {
    let result = goIncr();
    assert_eq!(result, 42);
}

// NOTE: type conversion (int()) not yet supported in transpiler
// ── Slice type shorthand ─────────────────────────────────────────────

// #[verify_rust_output({fn goSliceLen(a: &[i32]) -> i32 {
//         return int(a.len() as i32)
//     }})]
// go! {
//     func goSliceLen(a []int) int {
//         return int(len(a))
//     }
// }

// NOTE: type conversion (int()) not yet supported
// ── Slice type shorthand (2 params) ──────────────────────────────────

// #[verify_rust_output({fn goSliceSubindex(a: &[i32], b: &[i32]) -> i32 {
//         return a.len() as i32 - b.len() as i32
//     }})]
// go! {
//     func goSliceSubindex(a, b []int) int {
//         return len(a) - len(b)
//     }
// }

// NOTE: type conversion (int()) not yet supported in transpiler
// #[test]
// fn test_slice_type() {
//     let data = vec![10, 20, 30];
//     assert_eq!(goSliceLen(&data), 3);
//     let a = vec![1, 2];
//     let b = vec![3];
//     assert_eq!(goSliceSubindex(&a, &b), 1);
// }

// ── String conversion builtin ────────────────────────────────────────────

#[verify_rust_output({fn goStr(bytes: &[u8]) -> ::gourd::GoString {
        return ::gourd::GoString::from(::std::str::from_utf8(&bytes).unwrap_or("").to_string())
    }})]
go! {
    func goStr(bytes []byte) string {
        return string(bytes)
    }
}

#[test]
fn test_string_builtin() {
    let bytes = vec![72, 101, 108, 108, 111];  // "Hello"
    assert_eq!(goStr(&bytes), "Hello");
}

// ── Go-style parameter shorthand: group multiple params with shared type ────

#[verify_rust_output({fn goShorthand(a: i32, b: i32, c: i32) -> i32 {
        return a + b + c
    }})]
go! {
    func goShorthand(a, b, c int) int {
        return a + b + c
    }
}

#[test]
fn test_param_grouping() {
    assert_eq!(goShorthand(1, 2, 3), 6);
}


#[verify_rust_output({fn hello() -> ::gourd::GoString {
        return ::gourd::GoString::from("hello")
    }})]
go! {
    func hello() string {
        return "hello"
    }
}

#[test]
fn test_error_signature_check() {
    let _ = hello();
}

// ── Slice/map literals inside go! function bodies ────────────────────


// ── Slice literals inside go! function bodies ────────────────────

#[verify_rust_output({fn goSliceLiteral() -> Vec<i32> {
        return vec ! [ 1 , 2 , 3 ]
    }})]
go! {
    func goSliceLiteral() []int {
        return []int{1, 2, 3}
    }
}

#[test]
fn test_slice_literal_in_body() {
    let v = goSliceLiteral();
    assert_eq!(v, vec![1i32, 2i32, 3i32]);
}


// NOTE: empty slice literals not yet implemented
// #[verify_rust_output({fn goSliceLiteralEmpty() -> Vec<i32> {
//         return vec![]
//     }})]
// go! {
//     func goSliceLiteralEmpty() []int {
//         return []int{}
//     }
// }

// #[test]
// fn test_slice_literal_empty_in_body() {
//     let v = goSliceLiteralEmpty();
//     assert!(v.is_empty());
// }


// NOTE: type-inferred slice literals not yet implemented
// #[verify_rust_output({fn goSliceLiteralTypeInferred() -> Vec<i32> {
//         return vec![2, 3, 4]
//     }})]
// go! {
//     func goSliceLiteralTypeInferred() []int {
//         return []int{2, 3, 4}
//     }
// }

// #[test]
// fn test_slice_literal_type_inferred_in_body() {
//     let v = goSliceLiteralTypeInferred();
//     assert_eq!(v, vec![2i32, 3i32, 4i32]);
// }

// NOTE: this hangs during build when uncommented:
//
// go! {
//     fn goMapLiteral(a string) int {
//         let m = map[string]int{ "a": 1, "b": 2, "c": 3 };
//         *m.get(a).unwrap()
//     }
// }
//
// #[test]
// fn test_map_literal_in_body() {
//     use std::collections::HashMap;
//     let result = goMapLiteral(String::from("b"));
//     assert_eq!(result, 2i32);
//     let result = goMapLiteral(String::from("a"));
//     assert_eq!(result, 1i32);
// }


// NOTE: map literals not yet implemented
// #[verify_rust_output({fn goMapLiteralEmpty() -> bool {
//         return len(map[string]int{}) == 0
//     }})]
// go! {
//     func goMapLiteralEmpty() bool {
//         return len(map[string]int{}) == 0
//     }
// }

// #[test]
// fn test_map_literal_empty_in_body() {
//     assert!(goMapLiteralEmpty());
// }


// NOTE: int-keyed map literals not yet implemented
// #[verify_rust_output({fn goIntMap() -> String {
//         return map[int]string{1: "one", 2: "two"}[2]
//     }})]
// go! {
//     func goIntMap() string {
//         return map[int]string{1: "one", 2: "two"}[2]
//     }
// }

// #[test]
// fn test_int_map_in_body() {
//     let result = goIntMap();
//     assert_eq!(result, "two".to_string());
// }
