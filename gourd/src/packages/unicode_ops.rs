//! Go's `unicode` package helpers.
//!
//! Provides character type utilities matching Go's unicode stdlib.

/// Checks if a character is a letter (Go `unicode.IsLetter`).
pub fn is_letter(c: char) -> bool {
    c.is_alphabetic()
}

/// Checks if a character is a digit (Go `unicode.IsDigit`).
pub fn is_digit(c: char) -> bool {
    c.is_ascii_digit() || c.to_string().chars().all(|c| c.is_ascii_digit())
}

/// Checks if a character is a lowercase letter (Go `unicode.IsLower`).
pub fn is_lower(c: char) -> bool {
    c.is_lowercase()
}

/// Checks if a character is an uppercase letter (Go `unicode.IsUpper`).
pub fn is_upper(c: char) -> bool {
    c.is_uppercase()
}

/// Checks if a character is a space (Go `unicode.IsSpace`).
pub fn is_space(c: char) -> bool {
    c.is_whitespace()
}

/// Checks if a character is a control character (Go `unicode.IsControl`).
pub fn is_control(c: char) -> bool {
    c.is_control()
}

/// Converts a character to uppercase (Go `unicode.ToUpper`).
pub fn to_upper(c: char) -> char {
    c.to_uppercase().next().unwrap_or(c)
}

/// Converts a character to lowercase (Go `unicode.ToLower`).
pub fn to_lower(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Checks if a character is a valid identifier character (Go `unicode.IsLetter` or `unicode.IsDigit`).
pub fn is_valid(c: char) -> bool {
    c.is_alphabetic() || c.is_ascii_digit() || c == '_'
}
