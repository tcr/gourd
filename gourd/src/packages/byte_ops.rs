//! Go's byte/rune operations.
//!
//! Provides byte and rune conversion helpers.

/// Returns the byte representation of a character (Go `byte(char)`).
pub fn byte_of(c: char) -> u8 {
    // Only returns the first byte if the char is ASCII
    c as u8
}

/// Returns the rune (Unicode code point) from a byte (Go `rune(byte)`).
pub fn rune_of(b: u8) -> char {
    b as char
}

/// Converts a string to bytes (Go `[]byte(string)`).
pub fn string_to_bytes(s: &str) -> Vec<u8> {
    s.as_bytes().to_vec()
}

/// Converts bytes to a string (Go `string([]byte)`).
pub fn bytes_to_string(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}
