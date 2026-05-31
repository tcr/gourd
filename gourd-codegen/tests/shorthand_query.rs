use gourd_codegen::{go, verify_rust_output};

// Test 3 params (2 group commas)

#[verify_rust_output({ fn go_shorthand_2(a: i32, b: i32) -> i32 { a + b } })]
go! {
    fn go_shorthand_2(a, b int) int {
        a + b
    }
}

// Test 3 params (2 group commas) — should also work if group parsing is correct

#[verify_rust_output({ fn go_shorthand_3(a: i32, b: i32, c: i32) -> i32 { a + b + c } })]
go! {
    fn go_shorthand_3(a, b, c int) int {
        a + b + c
    }
}

#[test]
fn test_param_grouping() {
    assert_eq!(go_shorthand_2(1, 2), 3);
}

#[test]
fn test_param_grouping_3() {
    assert_eq!(go_shorthand_3(1, 2, 3), 6);
}
