//! Tests for Go channel operations: `<- ch` (receive) and `ch <- value` (send).

use gourd_macro::go;
use gourd::{GoChannel, GoScheduler};

// Test channel receive: `<- ch` → `ch.recv()`
#[test]
fn test_channel_recv() {
    go! {
        func goChannelRecv(ch chan int) int {
            return <-ch
        }
    }
}

// Test channel send: `ch <- value` → `ch.send(value)`
#[test]
fn test_channel_send() {
    go! {
        func goChannelSend(ch chan int, value int) {
            ch <- value
        }
    }
}

// Test both send and receive in the same function
#[test]
fn test_channel_send_and_recv() {
    go! {
        func goChannelEcho(ch chan int) int {
            ch <- 42
            return <-ch
        }
    }
}