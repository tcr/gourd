use gourd_macro::{go, verify_rust_output};

// Test 3 params (2 group commas)
// Go name `goShorthand2` is preserved as camelCase

#[verify_rust_output({ fn goShorthand2(a: i32, b: i32) -> i32 { return a + b } })]
go! {
    func goShorthand2(a, b int) int {
        return a + b
    }
}

// Test 3 params (2 group commas) — should also work if group parsing is correct
// Go name `goShorthand3` is preserved as camelCase

#[verify_rust_output({ fn goShorthand3(a: i32, b: i32, c: i32) -> i32 { return a + b + c } })]
go! {
    func goShorthand3(a, b, c int) int {
        return a + b + c
    }
}

#[test]
fn test_param_grouping() {
    assert_eq!(goShorthand2(1, 2), 3);
}

#[test]
fn test_param_grouping_3() {
    assert_eq!(goShorthand3(1, 2, 3), 6);
}
