use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use gourd_codegen::go;

    // This binary shows what `go! { ... }` actually expands to:
    // The macro outputs exactly equivalent Rust expressions.

    // Arithmetic tests
    let sum: i32 = go! { 10 + 20 };
    println!("10 + 20 = {sum}");
    assert_eq!(sum, 30);

    let diff: i32 = go! { 50 - 10 };
    println!("50 - 10 = {diff}");
    assert_eq!(diff, 40);

    let prod: i32 = go! { 4 * 5 };
    println!("4 * 5 = {prod}");
    assert_eq!(prod, 20);

    let quot: i32 = go! { 100 / 4 };
    println!("100 / 4 = {quot}");
    assert_eq!(quot, 25);

    // Parenthesized expression
    let parens: i32 = go! { (3 + 2) * 4 };
    println!("(3 + 2) * 4 = {parens}");
    assert_eq!(parens, 20);

    // Negation
    let neg: i32 = go! { -7 + 3 };
    println!("-7 + 3 = {neg}");
    assert_eq!(neg, -4);

    // Short-circuit boolean logic
    let bool_test: bool = go! { true && false };
    println!("true && false = {bool_test}");
    assert!(!bool_test);

    println!("All Go→Rust transpilation results verified!");

    Ok(())
}
