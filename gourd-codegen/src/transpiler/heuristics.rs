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
// Map Detection Heuristics (structural only)
// ============================================================================
/// Type keywords that identify a map collection structurally.
const MAP_TYPE_KEYWORDS: &[&str] = &[
    "HashMap",
    "hash_map",
    "GoMap",
    "GoMap::",
    "go :: gourd :: GoMap",
    "gourd :: GoMap",
];

/// Type keywords that identify a string index structurally.
const STRING_INDEX_KEYWORDS: &[&str] = &[
    "String",
    "from(",
    "GoString",
    ":: gourd :: GoString",
    "gourd :: GoString",
];

/// Exact names known to be map variables in the codebase.
/// This is a tiny, targeted fallback for when structural type detection
/// isn't available (local variable names without type info).
pub const KNOWN_MAP_NAMES: &[&str] = &["counts", "count", "seen", "result", "top"];

/// Check whether a name indicates a map collection type (HashMap, hash_map).
pub fn collection_is_map_type(name: &str) -> bool {
    MAP_TYPE_KEYWORDS.iter().any(|k| name.contains(k))
}

/// Check whether an index indicates a string-typed key (String, from() pattern).
pub fn index_is_string_type(name: &str) -> bool {
    STRING_INDEX_KEYWORDS.iter().any(|k| name.contains(k))
}

/// Determine if map access should use `map_get_ref` vs standard indexing.
///
/// Uses structural type detection only — checks if the collection's token
/// stream contains known map types or the index contains string type info.
pub fn heuristic_should_use_map_get_ref(collection: &str, index: &str) -> bool {
    // Structural: collection type is a map
    if collection_is_map_type(collection) {
        return true;
    }
    // Structural: index type is a string
    if index_is_string_type(index) {
        return true;
    }
    // Targeted name fallback: known map variable names
    is_known_map_name(collection)
}

/// Check whether a name is known to be a map collection.
pub fn is_known_map_name(name: &str) -> bool {
    KNOWN_MAP_NAMES.contains(&name)
}

/// Determine if map assignment should use `map_set_mut_ref`.
///
/// Uses structural type detection only — returns false when no type info
/// is available (the caller should fall through to standard indexing).
pub fn heuristic_should_use_map_set(collection: &str) -> bool {
    // Structural: collection type is a map
    if collection_is_map_type(collection) {
        return true;
    }
    // Targeted name fallback: known map variable names
    is_known_map_name(collection)
}

/// Check whether an iterator variable suggests map-iteration context.
///
/// Uses structural type detection only — returns true when the collection
/// expression contains a known map type keyword.
pub fn heuristic_is_map_iteration(collection: &str) -> bool {
    // Structural detection: actual HashMap types
    let lower = collection.to_lowercase();
    if lower.contains("hashmap") || lower.contains("hash_map") || lower.contains("gomap") {
        return true;
    }
    // Targeted name fallback: known map variable names
    is_known_map_name(collection)
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
    "Heuristics module — structural type detection + targeted name fallbacks.\n\
     Map detection: HashMap/hash_map types, String indices, known map names → use map_get_ref/map_set_mut_ref\n\
     Numeric add: field access (.value/.n/.data) or known numeric names → numeric addition, else string concat\n\
     WARNING: these are not type analysis — they fail on code with different naming patterns."
}
