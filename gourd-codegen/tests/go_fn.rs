use gourd_codegen::go;

// ── Basic function: no params, return value ─────────────────────────
go! {
    fn go_add() -> i32 {
        42
    }
}

// ── Function with mapped parameter type names ───────────────────────
go! {
    fn go_sum(a: i32, b: i32) -> i32 {
        a + b
    }
}

// ── Function with internal (returns), ───────────────────────────────
go! {
    fn go_abs(n: i32) -> i32 {
        let mut ret = 0i32;
        if n < 0 {
            ret = -n;
        } else {
            ret = n;
        }
        ret
    }
}

// ── Boolean return ─────────────────────────────────────────────────
go! {
    fn is_even(n: i32) -> bool {
        n % 2 == 0
    }
}

// ── Multiple return values ─────────────────────────────────────────
go! {
    fn go_divmod(n: i32, d: i32) -> (i32, i32) {
        (n / d, n % d)
    }
}

// ── String param ────────────────────────────────────────────────────
go! {
    fn go_len(s: String) -> i32 {
        s.len() as i32
    }
}

// ── No return ───────────────────────────────────────────────────────
go! {
    fn go_incr() -> i32 {
        42
    }
}

#[test]
fn test_fn_return() {
    assert_eq!(go_add(), 42);
}

#[test]
fn test_fn_with_params() {
    assert_eq!(go_sum(10, 20), 30);
}

#[test]
fn test_fn_if_return() {
    assert_eq!(go_abs(-5), 5);
    assert_eq!(go_abs(3), 3);
    assert_eq!(go_abs(0), 0);
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
fn test_fn_string_param() {
    assert_eq!(go_len(String::from("hello")), 5);
}

#[test]
fn test_fn_no_return() {
    let result = go_incr();
    assert_eq!(result, 42);
}
