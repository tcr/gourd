use gourd_codegen::go;

// Test 3 params (2 group commas)
go! {
    fn go_shorthand_2(a, b int) int {
        a + b
    }
}

// Test 3 params (2 group commas) — should also work if group parsing is correct
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
