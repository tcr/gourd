//! Tests Go's `recover()` builtin in closure bodies.

use gourd_macro::go;

// Test: recover without panic → nil
go! {
    func goRecoverNoPanic() string {
        result := recover()
        if result != nil {
            return "caught"
        }
        return "no panic"
    }
}

// Test: recover after panic in closure body
go! {
    func goRecoverAfterPanic() string {
        f := func() int {
            panic("oops")
            return 0
        }
        f()
        result := recover()
        if result != nil {
            return "caught"
        }
        return "no panic"
    }
}

#[test]
fn test_recover_no_panic() {
    assert_eq!(goRecoverNoPanic(), "no panic");
}

#[test]
fn test_recover_with_panic() {
    assert_eq!(goRecoverAfterPanic(), "no panic");
}
