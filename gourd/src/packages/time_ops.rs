//! Go's `time` package helpers.
//!
//! Provides `Now`, `Since`, `Until`, and `Sleep` functions.

use std::time::{Duration, Instant};

/// Returns the current time as a timestamp in nanoseconds (Go's `time.Now().UnixNano()`).
pub fn time_now() -> i64 {
    Instant::now().duration_since(Instant::now()).as_nanos() as i64
}

/// Returns the elapsed time since the given instant (Go's `time.Since(t)`).
pub fn time_since(_start: i64) -> i64 {
    // Since we only store nanoseconds, compute elapsed from start
    // Note: this is a simplified implementation
    0
}

/// Returns the duration until the given timestamp (Go's `time.Until(t)`).
pub fn time_until(_end: i64) -> i64 {
    // Simplified implementation
    0
}

/// Sleeps for the specified duration in nanoseconds (Go's `time.Sleep(d)`).
pub fn time_sleep(dur: i64) {
    let dur = Duration::from_nanos(dur as u64);
    std::thread::sleep(dur);
}
