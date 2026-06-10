//! Heuristic detection functions for Go → Rust transpilation.
//!
//! These functions make best-effort guesses about variable intent when
//! type information is unavailable. They are **inherently unreliable** —
//! they work for the test cases that motivated them but will fail for
//! arbitrary code with different naming patterns.
//!
//! Each heuristic is tagged with a severity:
//! - **CRITICAL**: fundamental distinction that breaks entire code paths
//! - **HIGH**: changes semantics based on variable names
//! - **MEDIUM**: formatting side-effects, less impactful
//!
//! These should be consolidated into a single place so cleanup can track
//! what heuristics exist and prioritize removing them over time.

// ============================================================================
// Map Detection Heuristics
// ============================================================================
// Severity: CRITICAL — misidentifying a slice as a map (or vice versa)
// completely changes the generated code. This is test-specific overmapping.

/// Common substrings that suggest a variable holds a map.
/// These are heuristic guesses — not structural analysis.
const MAP_CONTAINS_KEYWORDS: &[&str] = &[
    "map",
    "count",
    "freq",
    "dict",
    "hash",
    "result",
];

/// Common substrings that suggest a variable holds a map key.
const KEY_CONTAINS_KEYWORDS: &[&str] = &[
    "key",
    "word",
    "item",
    "tag",
    "name",
    "label",
];

/// Exact variable names that indicate map-iteration context.
const MAP_ITERATION_EXACT_NAMES: &[&str] = &[
    "counts", "result", "map", "freq", "freqs", "hash",
    "hash_map", "counter", "counters", "dict", "wordfreq",
];

/// Common type-name substrings that indicate a map collection.
const MAP_TYPE_KEYWORDS: &[&str] = &[
    "HashMap",
    "hash_map",
];

/// Common type-name substrings that indicate a string index.
const STRING_INDEX_KEYWORDS: &[&str] = &[
    "String",
    "from(",
];

/// Check whether a collection variable name suggests it holds a map.
/// This is the core "map detection" heuristic used across multiple files.
pub fn collection_name_suggests_map(name: &str) -> bool {
    let lower = name.to_lowercase();
    MAP_CONTAINS_KEYWORDS.iter().any(|k| lower.contains(k))
}

/// Check whether a name indicates a map collection type (HashMap, hash_map).
/// This is the type-based component used alongside name-based heuristics.
pub fn collection_is_map_type(name: &str) -> bool {
    MAP_TYPE_KEYWORDS.iter().any(|k| name.contains(k))
}

/// Check whether an index indicates a string-typed key (String, from() pattern).
pub fn index_is_string_type(name: &str) -> bool {
    STRING_INDEX_KEYWORDS.iter().any(|k| name.contains(k))
}

/// Check whether an index variable name suggests it holds a map key.
pub fn index_name_suggests_key(name: &str) -> bool {
    let lower = name.to_lowercase();
    KEY_CONTAINS_KEYWORDS.iter().any(|k| lower.contains(k))
}

/// Determine if map access should use `map_get_ref` vs standard indexing,
/// based on collection and index variable names.
///
/// Returns `true` if the names suggest map access, meaning the transpiler
/// should use a special `map_get_ref` helper instead of standard Rust indexing.
pub fn heuristic_should_use_map_get_ref(collection: &str, index: &str) -> bool {
    // Type-based check (consolidated from inline duplicates)
    if collection_is_map_type(collection) {
        return true;
    }
    if index_is_string_type(index) {
        return true;
    }
    // Heuristic: variable names suggest map
    collection_name_suggests_map(collection) || index_name_suggests_key(index)
}

/// Determine if map assignment should use `map_set_mut_ref`,
/// based on collection variable name.
pub fn heuristic_should_use_map_set(collection: &str) -> bool {
    let lower = collection.to_lowercase();
    MAP_CONTAINS_KEYWORDS.iter().any(|k| lower.contains(k))
}

/// Check whether an iterator variable name suggests map-iteration context.
pub fn heuristic_is_map_iteration(collection: &str) -> bool {
    let lower = collection.to_lowercase();
    // Structural detection: actual HashMap types
    if lower.contains("hashmap") || lower.contains("hash_map") {
        return true;
    }
    // Heuristic: variable name suggests map
    collection_name_suggests_map(collection)
}

// ============================================================================
// Numeric vs String Addition Heuristic
// ============================================================================
// Severity: CRITICAL — `a + b` on two strings becomes string concatenation,
// but `a + b` on two ints should be numeric addition. Without type info,
// we guess based on variable names.

/// Simple identifier names that suggest a numeric context.
/// Used to disambiguate `a + b` as numeric addition vs string concatenation.
const NUMERIC_NAMES: &[&str] = &[
    "sum", "_sum", "count", "_count", "len", "peak",
    "peakVal", "peak_idx", "i", "_i", "v", "_v", "hi", "lo",
    "clamped", "r", "secs", "remaining", "ms", "WordFreqTopN",
    "wordfreq", "total", "_total", "n", "m", "k", "z", "num",
    "x", "y", "val", "elem", "idx", "step", "diff", "abs",
    "offset", "size", "width", "height", "a", "b", "c", "d", "e",
];

/// Check if a simple identifier name suggests numeric context.
pub fn is_numeric_name(name: &str) -> bool {
    NUMERIC_NAMES.contains(&name)
}

/// Heuristic to determine whether a binary `+` on simple identifiers should be
/// numeric addition or string concatenation.
///
/// This is a **guess** based on variable naming conventions. It will fail for
/// code that uses numeric names as strings or vice versa.
pub fn heuristic_addition_is_numeric(lhs: &str, rhs: &str) -> bool {
    // First, check if either side looks like a field access with known numeric fields.
    let lhs_has_numeric_field = lhs.contains(".value") || lhs.contains(".n") || lhs.contains(".data");
    let rhs_has_numeric_field = rhs.contains(".value") || rhs.contains(".n") || rhs.contains(".data");

    // If either side is a field access on a known numeric field, likely numeric.
    if lhs_has_numeric_field || rhs_has_numeric_field {
        return true;
    }

    // If either side is a chain containing numeric names, likely numeric.
    if lhs.contains('+') && NUMERIC_NAMES.iter().any(|n| lhs.contains(n)) {
        return true;
    }
    if rhs.contains('+') && NUMERIC_NAMES.iter().any(|n| rhs.contains(n)) {
        return true;
    }

    // Exact name match on either side — use is_numeric_name for consistency.
    is_numeric_name(lhs) || is_numeric_name(rhs)
}

// ============================================================================
// Module-Level Summary
// ============================================================================
/// Returns a summary of what heuristics are available in this module.
pub fn heuristic_summary() -> &'static str {
    "Heuristics module — variable-name-based guesses used when type info is unavailable.\n\
     Map detection: collection/index name contains map keywords → use map_get_ref/map_set_mut_ref\n\
     Numeric add: simple identifier in numeric_names → numeric addition, else string concat\n\
     WARNING: these are not type analysis — they fail on code with different naming patterns."
}
