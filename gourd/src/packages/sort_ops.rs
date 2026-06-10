//! Go's `sort` package helpers.
//!
//! Provides slice sorting utilities matching Go's sort stdlib.

/// Sorts a slice in ascending order (Go `sort.Slice`).
pub fn sort<T: Ord>(slice: &mut [T]) {
    slice.sort();
}

/// Reverses a slice in place (Go `sort.Reverse`).
pub fn reverse<T>(slice: &mut [T]) {
    slice.reverse();
}

/// Returns true if the slice is sorted (Go `sort.IsSorted`).
pub fn is_sorted<T: Ord>(slice: &[T]) -> bool {
    slice.windows(2).all(|w| w[0] <= w[1])
}

/// Searches for a value in a sorted slice (Go `sort.Search`).
pub fn search<T: Ord, F: FnMut(usize) -> bool>(data: &[T], mut f: F) -> usize {
    let mut low = 0usize;
    let mut high = data.len();
    while low < high {
        let mid = low + (high - low) / 2;
        if f(mid) { high = mid; } else { low = mid + 1; }
    }
    low
}

/// Searches for a value in a sorted string slice (Go `sort.SearchStrings`).
pub fn search_strings(data: &[String], target: String) -> usize {
    search(data, |i| data[i] >= target)
}
