//! Go's `strings.Replace` helpers.
//!
//! Provides `Replace` and `ReplaceAll` functions.

/// Go's `strings.Replace(s, old, new, n)` — replaces occurrences of old with new.
/// n < 0 means replace all occurrences.
pub fn strings_replace(s: String, old: String, new: String, n: i32) -> String {
    if n < 0 {
        s.replace(&old, &new)
    } else {
        let mut result = s;
        for _ in 0..n.max(0) {
            match result.find(&old) {
                Some(pos) => {
                    result.replace_range(pos..pos + old.len(), &new);
                }
                None => break,
            }
        }
        result
    }
}

/// Go's `strings.ReplaceAll(s, old, new)` — replaces all occurrences of old with new.
pub fn strings_replace_all(s: String, old: String, new: String) -> String {
    s.replace(&old, &new)
}

/// Go's `strings.HasPrefix(s, prefix)` — checks if string starts with prefix.
pub fn has_prefix(s: String, prefix: String) -> bool {
    s.starts_with(&prefix)
}

/// Go's `strings.HasSuffix(s, suffix)` — checks if string ends with suffix.
pub fn has_suffix(s: String, suffix: String) -> bool {
    s.ends_with(&suffix)
}

/// Go's `strings.LastIndex(s, substr)` — returns last index of substring (-1 if not found).
pub fn last_index_str(s: String, substr: String) -> i32 {
    match s.rfind(&substr) {
        Some(pos) => pos as i32,
        None => -1,
    }
}

/// Go's `strings.Fields(s)` — splits string by whitespace, returning non-empty tokens.
pub fn fields(s: String) -> Vec<String> {
    s.split_whitespace().map(|s| s.to_string()).collect()
}
