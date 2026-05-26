fn main() -> Result<(), Box<dyn std::error::Error>> {
    use gourd::{GoGc, go_expr};

    // Arithmetic tests
    let sum: i32 = go_expr! { 10 + 20 };
    println!("10 + 20 = {sum}");
    assert_eq!(sum, 30);

    let diff: i32 = go_expr! { 50 - 10 };
    println!("50 - 10 = {diff}");
    assert_eq!(diff, 40);

    let prod: i32 = go_expr! { 4 * 5 };
    println!("4 * 5 = {prod}");
    assert_eq!(prod, 20);

    let quot: i32 = go_expr! { 100 / 4 };
    println!("100 / 4 = {quot}");
    assert_eq!(quot, 25);

    // Parenthesized expression
    let parens: i32 = go_expr! { (3 + 2) * 4 };
    println!("(3 + 2) * 4 = {parens}");
    assert_eq!(parens, 20);

    // Negation
    let neg: i32 = go_expr! { -7 + 3 };
    println!("-7 + 3 = {neg}");
    assert_eq!(neg, -4);

    // Short-circuit boolean logic
    let bool_test: bool = go_expr! { true && false };
    println!("true && false = {bool_test}");
    assert!(!bool_test);

    // GoGc runtime: heap-allocated, Arc-based shared ownership
    struct Point { x: i32, y: i32 }

    let p = GoGc::new(Point { x: 1, y: 2 });
    let _q = GoGc::clone(&p);
    
    assert_eq!(p.x, 1);
    assert_eq!(p.y, 2);
    println!("GoGc reference count (p.copied(q)): {}", p.strong_count());
    
    assert_eq!(p.strong_count(), 2);

    let _r = GoGc::clone(&p);
    println!("GoGc reference count (3x): {}", p.strong_count());
    assert_eq!(p.strong_count(), 3);

    let only = GoGc::new(Point { x: 99, y: 100 });
    match GoGc::try_unwrap(only) {
        Ok(Point { x, y }) => println!("try_unwrap succeeded: Point {{ x: {x}, y: {y} }}"),
        Err(_) => panic!("try_unwrap should succeed when refcount == 1"),
    }

    println!("All Go→Rust transpilation results verified!");
    println!("GoGc runtime: Arc-based reference counting working!");

    Ok(())
}
