//! Tests for Go `panic` builtin transpilation.
//!
//! - `panic("msg")` → `panic!("msg")`
//! - `panic()` → `panic!("panic()")`

use gourd_macro::go;

// Module-level `go!` blocks so functions are in scope for the tests

// Test: panic with a string message
go! {
    func goPanicMsg() int {
        panic("something went wrong")
        return 0
    }
}

// Test: panic with no argument
go! {
    func goPanicEmpty() int {
        panic()
        return 0
    }
}

// Test: panic inside an if block
go! {
    func goPanicOnError(n int) int {
        if n < 0 {
            panic("negative number not allowed")
        }
        return n
    }
}

// Tests

#[test]
#[should_panic(expected = "something went wrong")]
fn test_panic_with_message() {
    let _ = goPanicMsg();
}

#[test]
#[should_panic]
fn test_panic_no_arg() {
    let _ = goPanicEmpty();
}

#[test]
fn test_panic_conditional() {
    // Normal case: should not panic
    assert_eq!(goPanicOnError(5), 5);
}

#[test]
#[should_panic(expected = "negative number not allowed")]
fn test_panic_negative_input() {
    let _ = goPanicOnError(-1);
}
