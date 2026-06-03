//! Utility functions for transpilation.
//!
//! Provides helper functions like name conversion.

/// Convert a Go name (camelCase) to Rust snake_case.
/// `goAdd` → `go_add`, `goShorthand2` → `go_shorthand_2`
/// Handles consecutive caps and trailing digits.
pub fn to_snake_case(name: &str) -> String {
    let mut result = String::with_capacity(name.len() + 4);
    let chars: Vec<char> = name.chars().collect();
    for (i, ch) in chars.iter().enumerate() {
        if ch.is_uppercase() {
            if i > 0 && !name[..i].ends_with('_') {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
        } else if ch.is_ascii_digit() && i > 0 && chars[i - 1].is_lowercase() {
            // Add underscore before digit if preceded by lowercase
            result.push('_');
            result.push(*ch);
        } else {
            result.push(*ch);
        }
    }
    result
}
