//! Tests for Go `append` builtin transpilation.

use gourd_macro::go;

// Module-level `go!` blocks so functions are in scope for the tests

// Test: append a single item
go! {
    func goAppendItem() []int {
        return append([]int{1, 2, 3}, 4)
    }
}

// Test: append multiple items
go! {
    func goAppendMultiple() []int {
        return append([]int{1}, 2, 3, 4)
    }
}

// Test: append with no items (no-op)
go! {
    func goAppendNoop() []int {
        return append([]int{1, 2})
    }
}

// Test: append to a variable slice
go! {
    func goAppendVar(data []int, x int) []int {
        return append(data, x)
    }
}

// Tests

#[test]
fn test_append_single_item() {
    let result = goAppendItem();
    assert_eq!(result, vec![1, 2, 3, 4]);
}

#[test]
fn test_append_multiple_items() {
    let result = goAppendMultiple();
    assert_eq!(result, vec![1, 2, 3, 4]);
}

#[test]
fn test_append_noop() {
    let result = goAppendNoop();
    assert_eq!(result, vec![1, 2]);
}

#[test]
fn test_append_var() {
    let data = vec![10, 20];
    let result = goAppendVar(&data, 30);
    assert_eq!(result, vec![10, 20, 30]);
}
