use gourd::GoGc;
use gourd::go;

go! {
    fn go_add() -> i32 {
        10 + 20
    }
}

go! {
    fn go_sub() -> i32 {
        50 - 10
    }
}

go! {
    fn go_mul() -> i32 {
        4 * 5
    }
}

go! {
    fn go_div() -> i32 {
        100 / 4
    }
}

go! {
    fn go_parens() -> i32 {
        (3 + 2) * 4
    }
}

go! {
    fn go_neg() -> i32 {
        -7 + 3
    }
}

go! {
    fn go_bool() -> bool {
        true && false
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Arithmetic tests via go!
    let sum = go_add();
    println!("10 + 20 = {sum}");
    assert_eq!(sum, 30);

    let diff = go_sub();
    println!("50 - 10 = {diff}");
    assert_eq!(diff, 40);

    let prod = go_mul();
    println!("4 * 5 = {prod}");
    assert_eq!(prod, 20);

    let quot = go_div();
    println!("100 / 4 = {quot}");
    assert_eq!(quot, 25);

    let parens = go_parens();
    println!("(3 + 2) * 4 = {parens}");
    assert_eq!(parens, 20);

    let neg = go_neg();
    println!("-7 + 3 = {neg}");
    assert_eq!(neg, -4);

    let bool_test = go_bool();
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