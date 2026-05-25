use gourd_codegen::go;

// ── Expression nodes (currently supported) ──────────────────────────

#[test]
fn binary_integers() {
    assert_eq!(go! { 10 + 20 }, 30i32);
    assert_eq!(go! { 50 - 10 }, 40i32);
    assert_eq!(go! { 4 * 5 }, 20i32);
    assert_eq!(go! { 100 / 4 }, 25i32);
    assert_eq!(go! { 11 % 3 }, 2i32);
    assert_eq!(go! { 5 == 5 }, true);
    assert_eq!(go! { 3 < 5 }, true);
    assert_eq!(go! { 7 >= 5 }, true);
    assert_eq!(go! { 5 != 3 }, true);
}

#[test]
fn bitwise_ops() {
    assert_eq!(go! { 0b1100 | 0b0101 }, 0b1101u8);
    assert_eq!(go! { 0b1100 & 0b0101 }, 0b0100u8);
    assert_eq!(go! { 0b1100 ^ 0b0101 }, 0b1001u8);
    assert_eq!(go! { 2 << 3 }, 16i32);
    assert_eq!(go! { 64 >> 3 }, 8i32);
}

#[test]
fn boolean_mixed() {
    assert_eq!(go! { true && true }, true);
    assert_eq!(go! { true || false }, true);
    assert_eq!(go! { !false }, true);
}

#[test]
fn negation() {
    assert_eq!(go! { -7i32 }, -7i32);
    assert_eq!(go! { -(-3i32) }, 3i32);
}

#[test]
fn parentheses() {
    assert_eq!(go! { ( 10 + (20 * 3) ) }, 70i32);
}

// ── Path-based constants ────────────────────────────────────────────

#[test]
fn path_nil() {
    let v: Option<i32> = go! { nil };
    assert!(v.is_none());
}

#[test]
fn path_bool() {
    assert_eq!(go! { true }, true);
    assert_eq!(go! { false }, false);
}

// ── Short variable declarations (:=) ────────────────────────────────

#[test]
fn short_decl_let() {
    let result: i32 = go! { 42 };
    assert_eq!(result, 42i32);
}

// ── Calls ───────────────────────────────────────────────────────────

#[test]
fn len_call() {
    let v: Vec<i32> = vec![1, 2, 3];
    assert_eq!(go! { len(v) }, 3usize);
}

#[test]
fn cap_call() {
    let v: Vec<i32> = vec![1, 2, 3];
    assert_eq!(go! { cap(v) }, 3usize);
}

#[test]
fn ordinary_call() {
    let r = go! { String::from("hello") };
    assert_eq!(r.as_str(), "hello");
}

// ── If expressions ──────────────────────────────────────────────────

#[test]
fn if_else() {
    assert_eq!(go! { if true { 1 } else { 2 } }, 1i32);
    assert_eq!(go! { if false { 1 } else { 2 } }, 2i32);
}

#[test]
fn if_without_else() {
    go! { if true { drop(42i32) }};
}

// ── Index expressions ───────────────────────────────────────────────

#[test]
fn index_vec() {
    let v: Vec<i32> = vec![10, 20, 30];
    assert_eq!(go! { v[0] }, 10i32);
    assert_eq!(go! { v[2] }, 30i32);
}

#[test]
fn index_string() {
    let s: Vec<u8> = b"hello".to_vec();
    assert_eq!(go! { s[4] }, b'o');
}

#[test]
fn index_nested() {
    let m: Vec<Vec<i32>> = vec![vec![1, 2], vec![3, 4]];
    assert_eq!(go! { m[1][0] }, 3i32);
}

#[test]
fn index_in_if() {
    let v: Vec<i32> = vec![5, 10, 15];
    assert_eq!(go! { if v[0] < v[1] { v[1] } else { v[0] } }, 10i32);
}

// ── Field access:  Go pt.Field → Rust pt.Field (identical syntax) ──

#[test]
fn field_access() {
    let pt = (10, 20);
    assert_eq!(pt.0, go! { (pt).0 });
    assert_eq!(pt.1, go! { (pt).1 });
}

// Ranged patterns go! { for-range } are control-flow statements.
// Single-line field access transpiles: go! { pt.Field } → pt.Field

#[test]
fn if_else_value() {
    assert_eq!(go! { if true { 1 } else { 2 } }, 1i32);
    assert_eq!(go! { if false { 1 } else { 2 } }, 2i32);
}
