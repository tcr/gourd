//! Tests for Go `make` builtin transpilation.
//!
//! - `make(chan T)` → `GoChannel::<T>::new()`
//! - `make(chan T, cap)` → `GoChannel::<T>::with_capacity(cap)`
//! - `make(map[K]V)` → `HashMap::new()`
//! - `make([]T, len)` → `vec![0; len]`

use gourd_macro::go;
use gourd::GoChannel;

// Test: make(chan int) → GoChannel::<i32>::new()
#[test]
fn test_make_unbuffered_channel() {
    go! {
        func goMakeChannel() chan int {
            return make(chan int)
        }
    }
}

// Test: make(chan int, 10) → GoChannel::<i32>::with_capacity(10)
#[test]
fn test_make_buffered_channel() {
    go! {
        func goMakeBufferedChannel() chan int {
            return make(chan int, 10)
        }
    }
}

// Test: make(chan string, 5) → GoChannel::<String>::with_capacity(5)
#[test]
fn test_make_buffered_string_channel() {
    go! {
        func goMakeStringChannel() chan string {
            return make(chan string, 5)
        }
    }
}

// Test: make(map[string]int) → HashMap::new()
#[test]
fn test_make_map() {
    go! {
        func goMakeMap() map[string]int {
            return make(map[string]int)
        }
    }
}

// Test: make([]int, 5) → vec of zeros
#[test]
fn test_make_slice() {
    go! {
        func goMakeSlice() []int {
            return make([]int, 5)
        }
    }
}
