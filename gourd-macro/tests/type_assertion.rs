//! Type assertion tests: `x.(T)` → `x as T`
//!
//! Go type assertions like `x.(int)` are transpiled to Rust `as` casts.

use gourd_macro::go;

#[test]
fn test_type_assertion_int() {
    go! {
        func goTypeAssertInt(x int) int {
            return x.(int)
        }
    }
    
    // Verify the transpilation works at runtime
    assert_eq!(go_type_assert_int(42), 42);
}

#[test]
fn test_type_assertion_string() {
    go! {
        func goTypeAssertString(x int) String {
            return x.(string)
        }
    }
    
    // Verify the transpilation works at runtime
    assert_eq!(go_type_assert_string(42), String::from("42"));
}

#[test]
fn test_type_assertion_float64() {
    go! {
        func goTypeAssertFloat64(x int) f64 {
            return x.(float64)
        }
    }
    
    // Verify the transpilation works at runtime
    assert_eq!(go_type_assert_float_64(42), 42.0);
}

#[test]
fn test_type_assertion_uint() {
    go! {
        func goTypeAssertUint(x int) uint {
            return x.(uint)
        }
    }
    
    // Verify the transpilation works at runtime
    assert_eq!(go_type_assert_uint(-1), 4294967295u32);
}

#[test]
fn test_type_assertion_bool() {
    go! {
        func goTypeAssertBool(x int) bool {
            return x.(bool)
        }
    }
    
    // Verify the transpilation works at runtime
    assert_eq!(go_type_assert_bool(1), true);
}

#[test]
fn test_type_assertion_byte() {
    go! {
        func goTypeAssertByte(x int) byte {
            return x.(byte)
        }
    }
    
    // Verify the transpilation works at runtime
    assert_eq!(go_type_assert_byte(255), 255u8);
}

#[test]
fn test_type_assertion_rune() {
    go! {
        func goTypeAssertRune(x int) rune {
            return x.(rune)
        }
    }
    
    // Verify the transpilation works at runtime
    assert_eq!(go_type_assert_rune(65), 'A');
}

#[test]
fn test_type_assertion_nested() {
    go! {
        func goNestedTypeAssert(x int) int {
            return x.(int).(int)
        }
    }
    
    // Verify the transpilation works at runtime
    assert_eq!(go_nested_type_assert(42), 42);
}