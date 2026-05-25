use gourd_codegen::go_expr;

#[test]
fn simple_add() {
    let result = go_expr! { 10 + 20 };
    assert_eq!(result, 30i32);
}

#[test]
fn subtraction() {
    let result = go_expr! { 50 - 10 };
    assert_eq!(result, 40i32);
}

#[test]
fn multiplication() {
    let result = go_expr! { 4 * 5 };
    assert_eq!(result, 20i32);
}

#[test]
fn division() {
    let result = go_expr! { 100 / 4 };
    assert_eq!(result, 25i32);
}
