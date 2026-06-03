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
    assert_eq!(goTypeAssertInt(42), 42);
}

#[test]
fn test_type_assertion_string() {
    go! {
        func goTypeAssertString(x int) String {
            return x.(string)
        }
    }
    
    // Verify the transpilation works at runtime
    assert_eq!(goTypeAssertString(42), String::from("42"));
}

#[test]
fn test_type_assertion_float64() {
    go! {
        func goTypeAssertFloat64(x int) f64 {
            return x.(float64)
        }
    }
    
    // Verify the transpilation works at runtime
    assert_eq!(goTypeAssertFloat64(42), 42.0);
}

#[test]
fn test_type_assertion_uint() {
    go! {
        func goTypeAssertUint(x int) uint {
            return x.(uint)
        }
    }
    
    // Verify the transpilation works at runtime
    assert_eq!(goTypeAssertUint(-1), 4294967295u32);
}

#[test]
fn test_type_assertion_bool() {
    go! {
        func goTypeAssertBool(x int) bool {
            return x.(bool)
        }
    }
    
    // Verify the transpilation works at runtime
    assert_eq!(goTypeAssertBool(1), true);
}

#[test]
fn test_type_assertion_byte() {
    go! {
        func goTypeAssertByte(x int) byte {
            return x.(byte)
        }
    }
    
    // Verify the transpilation works at runtime
    assert_eq!(goTypeAssertByte(255), 255u8);
}

#[test]
fn test_type_assertion_rune() {
    go! {
        func goTypeAssertRune(x int) rune {
            return x.(rune)
        }
    }
    
    // Verify the transpilation works at runtime
    assert_eq!(goTypeAssertRune(65), 'A');
}

#[test]
fn test_type_assertion_nested() {
    go! {
        func goNestedTypeAssert(x int) int {
            return x.(int).(int)
        }
    }
    
    // Verify the transpilation works at runtime
    assert_eq!(goNestedTypeAssert(42), 42);
}