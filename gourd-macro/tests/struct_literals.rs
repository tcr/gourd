//! Tests for Go struct literal transpilation.
//!
//! `Point{x: 1, y: 2}` → `Point { x: 1, y: 2 }`
//!
//! These tests verify that struct literals are transpiled without hitting
//! the `compile_error!` TODO fallback.

use gourd_macro::go;

// Define types needed by the Go blocks below.
#[derive(Default)]
struct Point { pub x: i32, pub y: i32 }
struct Outer { pub inner: Inner }
struct Inner { pub x: i32 }

#[test]
fn test_struct_literal_in_return() {
    go! {
        func goStructReturn() Point { return Point{x: 1, y: 2} }
    }
}

// NOTE: empty struct literal `Point{}` → `Point {}` in Rust.
// This requires the type to have no fields or Default impl + `..Default::default()`.
// Not adding a runtime test here since the transpilation is verified by compilation
// of other tests.

// NOTE: Go partial struct literals like `Point{x: 1}` (omitting field y)
// don't have a direct Rust equivalent. Go fills omitted fields with zero values.
// The transpiler should emit `compile_error!` for partial struct literals,
// suggesting the user specify all fields explicitly.
// Planned for future: when the transpiler knows the struct type, it can
// automatically add `..Default::default()` for omitted fields.

#[test]
fn test_struct_literal_nested() {
    go! {
        func goNestedStructs() Outer { return Outer{inner: Inner{x: 1}} }
    }
}

#[test]
fn test_struct_literal_in_assignment() {
    go! {
        func goStructInLet() string {
            p := Point{x: 1, y: 2}
            return "ok"
        }
    }
}
