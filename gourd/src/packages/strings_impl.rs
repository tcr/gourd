//! Go's `strings.Replace` helpers.
//!
//! Provides `Replace` and `ReplaceAll` functions.

use crate::GoString;

/// Go's `strings.Replace(s, old, new, n)` — replaces occurrences of old with new.
/// n < 0 means replace all occurrences.
pub fn strings_replace<S: AsRef<str>>(s: S, old: S, new: S, n: i32) -> GoString {
    let s = s.as_ref();
    let old = old.as_ref();
    let new = new.as_ref();
    if n < 0 {
        GoString::from(&s.replace(old, new))
    } else {
        let mut result = s.to_string();
        for _ in 0..n.max(0) {
            match result.find(old) {
                Some(pos) => {
                    result.replace_range(pos..pos + old.len(), new);
                }
                None => break,
            }
        }
        GoString::from(&result)
    }
}

/// Go's `strings.ReplaceAll(s, old, new)` — replaces all occurrences of old with new.
pub fn strings_replace_all<S: AsRef<str>>(s: S, old: S, new: S) -> GoString {
    GoString::from(&s.as_ref().replace(old.as_ref(), new.as_ref()))
}

/// Go's `strings.HasPrefix(s, prefix)` — checks if string starts with prefix.
pub fn has_prefix<S: AsRef<str>>(s: S, prefix: S) -> bool {
    s.as_ref().starts_with(prefix.as_ref())
}

/// Go's `strings.HasSuffix(s, suffix)` — checks if string ends with suffix.
pub fn has_suffix<S: AsRef<str>>(s: S, suffix: S) -> bool {
    s.as_ref().ends_with(suffix.as_ref())
}

/// Go's `strings.LastIndex(s, substr)` — returns last index of substring (-1 if not found).
pub fn last_index_str<S: AsRef<str>>(s: S, substr: S) -> i32 {
    match s.as_ref().rfind(substr.as_ref()) {
        Some(pos) => pos as i32,
        None => -1,
    }
}

/// Go's `strings.Fields(s)` — splits string by whitespace, returning non-empty tokens.
pub fn fields<S: AsRef<str>>(s: S) -> Vec<GoString> {
    s.as_ref().split_whitespace().map(|s| GoString::from(s.to_string())).collect()
}
