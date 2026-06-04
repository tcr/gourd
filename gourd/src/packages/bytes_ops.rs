//! Go's `bytes` package helpers.
//!
//! Provides 7 byte slice operations matching Go's stdlib.

/// Go's `bytes.Contains(slice, substr)` — checks if slice contains substr.
pub fn bytes_contains(slice: &[u8], substr: &[u8]) -> bool {
    slice.windows(substr.len()).any(|w| w == substr)
}

/// Go's `bytes.HasPrefix(slice, prefix)` — checks if slice starts with prefix.
pub fn bytes_has_prefix(slice: &[u8], prefix: &[u8]) -> bool {
    slice.starts_with(prefix)
}

/// Go's `bytes.HasSuffix(slice, suffix)` — checks if slice ends with suffix.
pub fn bytes_has_suffix(slice: &[u8], suffix: &[u8]) -> bool {
    slice.ends_with(suffix)
}

/// Go's `bytes.Index(slice, substr)` — finds first occurrence of substr.
pub fn bytes_index(slice: &[u8], substr: &[u8]) -> i32 {
    slice
        .windows(substr.len())
        .position(|w| w == substr)
        .map(|i| i as i32)
        .unwrap_or(-1)
}

/// Go's `bytes.Split(slice, sep)` — splits slice by separator.
pub fn bytes_split(slice: &[u8], sep: &[u8]) -> Vec<Vec<u8>> {
    if sep.is_empty() {
        return vec![slice.to_vec()];
    }
    let mut result = Vec::new();
    let mut start = 0;
    while start <= slice.len() {
        if let Some(pos) = slice[start..].windows(sep.len()).position(|w| w == sep) {
            let end = start + pos;
            result.push(slice[start..end].to_vec());
            start = end + sep.len();
        } else {
            result.push(slice[start..].to_vec());
            break;
        }
    }
    result
}

/// Go's `bytes.Join(parts, sep)` — joins parts with separator.
pub fn bytes_join(parts: &[Vec<u8>], sep: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            result.extend_from_slice(sep);
        }
        result.extend_from_slice(part);
    }
    result
}

/// Go's `bytes.Replace(slice, old, new, n)` — replaces occurrences.
pub fn bytes_replace(slice: Vec<u8>, old: &[u8], new: &[u8], n: i32) -> Vec<u8> {
    if old.is_empty() {
        return slice;
    }
    let mut result = Vec::new();
    let mut count = 0;
    let mut pos = 0;
    while pos <= slice.len().saturating_sub(old.len()) && (n < 0 || count < n) {
        if slice[pos..pos + old.len()] == *old {
            result.extend_from_slice(new);
            count += 1;
            pos += old.len();
        } else {
            result.push(slice[pos]);
            pos += 1;
        }
    }
    result.extend_from_slice(&slice[pos..]);
    result
}
