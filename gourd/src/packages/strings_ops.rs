//! Go's `strings` package helpers.
//!
//! Provides 16 string manipulation functions matching Go's stdlib.

use crate::GoString;

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


/// Returns true if the slice contains the value (Go `Contains`).
pub fn contains<T: PartialEq>(slice: &[T], val: &T) -> bool {
    slice.contains(val)
}

/// Joins a vector of strings with a separator (Go `strings.Join`).
pub fn join(elems: impl AsRef<[GoString]>, sep: &str) -> GoString {
    elems.as_ref().iter().map(|e| e.as_ref()).collect::<Vec<_>>().join(sep).into()
}

/// Splits a string by a separator (Go `strings.Split`).
pub fn split<S: AsRef<str>>(s: S, sep: S) -> Vec<GoString> {
    s.as_ref().split(sep.as_ref()).map(|s| GoString::from(s.to_string())).collect()
}

/// Returns true if the string contains the substring (Go `strings.Contains`).
pub fn contains_str<S: AsRef<str>>(s: S, sub: S) -> bool {
    s.as_ref().contains(sub.as_ref())
}

/// Returns the first index of the substring, or -1 (Go `strings.Index`).
pub fn index_str<S: AsRef<str>>(s: S, sub: S) -> i32 {
    s.as_ref().find(sub.as_ref()).map(|i| i as i32).unwrap_or(-1)
}

/// Trims leading and trailing characters (Go `strings.Trim`).
pub fn trim<S: AsRef<str>, C: AsRef<str>>(s: S, cutset: C) -> GoString {
    GoString::from(s.as_ref().chars().filter(|c| !cutset.as_ref().contains(*c)).collect::<String>())
}

/// Trims leading characters (Go `strings.TrimLeft`).
pub fn trim_left<S: AsRef<str>, C: AsRef<str>>(s: S, cutset: C) -> GoString {
    GoString::from(s.as_ref().trim_start_matches(|c: char| cutset.as_ref().contains(c)).to_string())
}

/// Trims trailing characters (Go `strings.TrimRight`).
pub fn trim_right<S: AsRef<str>, C: AsRef<str>>(s: S, cutset: C) -> GoString {
    GoString::from(s.as_ref().trim_end_matches(|c: char| cutset.as_ref().contains(c)).to_string())
}

/// Converts a string to uppercase (Go `strings.ToUpper`).
pub fn to_upper<S: AsRef<str>>(s: S) -> GoString {
    GoString::from(s.as_ref().to_uppercase())
}

/// Converts a string to lowercase (Go `strings.ToLower`).
pub fn to_lower<S: AsRef<str>>(s: S) -> GoString {
    GoString::from(s.as_ref().to_lowercase())
}

/// Repeats a string n times (Go `strings.Repeat`).
pub fn repeat<S: AsRef<str>>(s: S, n: i32) -> String {
    if n <= 0 { return String::new(); }
    s.as_ref().repeat(n as usize)
}
