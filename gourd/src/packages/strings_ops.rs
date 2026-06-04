//! Go's `strings` package helpers.
//!
//! Provides 16 string manipulation functions matching Go's stdlib.

/// Returns the index of the first occurrence of `val` in a slice (-1 if not found).
pub fn index<T: PartialEq>(slice: &[T], val: &T) -> i32 {
    for (i, v) in slice.iter().enumerate() {
        if v == val { return i as i32; }
    }
    -1
}

/// Returns a sub-slice from start to end (Go slice[i:j]).
pub fn slice_sub<T: Clone>(slice: &[T], start: i32, end: i32) -> Vec<T> {
    let start = start.max(0) as usize;
    let end = end.max(0) as usize;
    let end = end.min(slice.len());
    if start >= end { return vec![]; }
    slice[start..end].to_vec()
}

/// Sorts a slice in ascending order (Go `sort.Slice`).
pub fn sort<T: Ord>(slice: &mut [T]) {
    slice.sort();
}

/// Reverses a slice in place (Go `sort.Reverse`).
pub fn reverse<T>(slice: &mut [T]) {
    slice.reverse();
}

/// Returns true if the slice contains the value (Go `Contains`).
pub fn contains<T: PartialEq>(slice: &[T], val: &T) -> bool {
    slice.contains(val)
}

/// Joins a slice of strings with a separator (Go `strings.Join`).
pub fn join<T: AsRef<str>>(elems: &[T], sep: &str) -> String {
    elems.iter().map(|e| e.as_ref()).collect::<Vec<&str>>().join(sep)
}

/// Splits a string by a separator (Go `strings.Split`).
pub fn split(s: &str, sep: &str) -> Vec<String> {
    s.split(sep).map(|s| s.to_string()).collect()
}

/// Returns true if the string contains the substring (Go `strings.Contains`).
pub fn contains_str(s: &str, sub: &str) -> bool {
    s.contains(sub)
}

/// Returns the first index of the substring, or -1 (Go `strings.Index`).
pub fn index_str(s: &str, sub: &str) -> i32 {
    s.find(sub).map(|i| i as i32).unwrap_or(-1)
}

/// Trims leading and trailing whitespace (Go `strings.TrimSpace`).
pub fn trim(s: &str) -> &str {
    s.trim()
}

/// Trims leading whitespace (Go `strings.TrimLeft`).
pub fn trim_left(s: &str) -> &str {
    s.trim_start()
}

/// Trims trailing whitespace (Go `strings.TrimRight`).
pub fn trim_right(s: &str) -> &str {
    s.trim_end()
}

/// Converts a string to uppercase (Go `strings.ToUpper`).
pub fn to_upper(s: &str) -> String {
    s.to_uppercase()
}

/// Converts a string to lowercase (Go `strings.ToLower`).
pub fn to_lower(s: &str) -> String {
    s.to_lowercase()
}

/// Repeats a string n times (Go `strings.Repeat`).
pub fn repeat(s: &str, n: i32) -> String {
    if n <= 0 { return String::new(); }
    s.repeat(n as usize)
}
