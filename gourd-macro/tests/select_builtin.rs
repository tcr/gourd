//! Tests for the `select` statement transpilation.

use gourd_macro::go;
use gourd::GoChannel;

// Test: select with only default case (non-blocking)
go! {
    func goSelectDefault(ch chan int) {
        select {
            default:
        }
    }
}

// Test: select with send case
go! {
    func goSelectSend(ch chan int, value int) {
        select {
            case ch <- value:
        }
    }
}

// Test: select with send and default
go! {
    func goSelectSendWithDefault(ch chan int, value int) {
        select {
            case ch <- value:
            default:
        }
    }
}

#[test]
fn test_select_default_compiles() {
    let ch = GoChannel::<i32>::new();
    goSelectDefault(ch);
}

#[test]
fn test_select_send_compiles() {
    // Use buffered channel so send succeeds immediately (no receiver needed)
    let ch = GoChannel::<i32>::with_capacity(1);
    goSelectSend(ch, 42);
}

#[test]
fn test_select_send_with_default_compiles() {
    let ch = GoChannel::<i32>::new();
    goSelectSendWithDefault(ch, 42);
}
