use gourd_codegen::go;

#[test]
fn simple_add() {
    let result = go! { 10 + 20 };
    assert_eq!(result, 30i32);
}

#[test]
fn subtraction() {
    let result = go! { 50 - 10 };
    assert_eq!(result, 40i32);
}

#[test]
fn multiplication() {
    let result = go! { 4 * 5 };
    assert_eq!(result, 20i32);
}

#[test]
fn division() {
    let result = go! { 100 / 4 };
    assert_eq!(result, 25i32);
}
