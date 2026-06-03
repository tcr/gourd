//! Tests for Go `new` builtin transpilation.
//!
//! - `new(Foo)` → `Foo::default()`

use gourd_macro::go;

#[derive(Default)]
struct Point {
    pub x: i32,
    pub y: i32,
}

// Test: new(Point) → Point::default()
#[test]
fn test_new_basic() {
    go! {
        func goNewPoint() Point {
            return new(Point)
        }
    }
}

// Test: new(Foo) in an assignment
#[test]
fn test_new_in_assignment() {
    go! {
        func goNewWithDefault() int {
            p := new(Point)
            return p.x
        }
    }
}

// Test: new(int) — basic type
#[test]
fn test_new_basic_type() {
    go! {
        func goNewInt() int {
            return new(int)
        }
    }
}

// Test: new(string)
#[test]
fn test_new_string_type() {
    go! {
        func goNewString() string {
            return new(string)
        }
    }
}
