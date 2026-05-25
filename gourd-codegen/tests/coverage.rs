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
