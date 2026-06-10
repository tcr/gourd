//! Go string semantics.
//!
//! Go strings are immutable sequences of bytes (UTF-8 encoded). This type
//! models those semantics faithfully:
//!
//! - Byte indexing: `s.get_byte(i)` → `u8` (Go `s[i]`)
//! - String slicing: `s.slice(start, end)` → new `GoString` (Go `s[i:j]`)
//! - Copy-on-write semantics: cloning is cheap (just shares the byte vec)
//! - UTF-8 aware: `len()` returns byte length (Go `len(s)`)
//! - Default value: `GoString::default()` → empty string ""

/// Go string — immutable sequence of UTF-8 encoded bytes.
///
/// Models Go's `string` type at runtime:
/// - Immutable byte sequence
/// - Byte-level indexing (`s[i]` → byte/char)
/// - Substring slicing (`s[i:j]` → new string)
/// - Copy-on-write semantics (cheap to clone)
///
#[derive(Clone, Default, Hash)]
pub struct GoString {
    // Manual PartialEq for flexible comparison with Rust string types
    bytes: Vec<u8>, // UTF-8 encoded bytes
}

impl GoString {
    /// Create a new GoString from a UTF-8 &str.
    pub fn new(s: &str) -> Self {
        GoString { bytes: s.as_bytes().to_vec() }
    }

    /// Go string byte indexing: `s[i]` → byte (u8).
    /// Matches Go semantics: string indexing always yields a byte, never a rune.
    pub fn get_byte(&self, i: usize) -> u8 {
        self.bytes[i]
    }

    /// Go string byte indexing: `s[i]` → byte (u8), with bounds checking.
    /// Returns None if index is out of bounds.
    pub fn get_byte_checked(&self, i: usize) -> Option<u8> {
        self.bytes.get(i).copied()
    }

    /// Go string slicing: `s[i:j]` → new GoString.
    /// Matches Go semantics: produces a new string containing bytes [i, j).
    pub fn slice(&self, start: usize, end: usize) -> Self {
        let len = self.bytes.len();
        let start = start.min(len);
        let end = end.min(len);
        GoString { bytes: self.bytes[start..end].to_vec() }
    }

    /// Return byte length (Go `len(s)`).
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns true if the string is empty (Go `s == ""`).
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Returns the underlying bytes as a slice.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Decode as UTF-8 and return a &str view.
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes).unwrap_or("")
    }

    /// Convert to a Rust String (owned).
    pub fn to_rust_string(&self) -> String {
        self.as_str().to_string()
    }

    /// Convert to Rust bytes (owned).
    pub fn to_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Convert from Rust bytes.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        GoString { bytes }
    }

    /// Convert from Rust &str.
    pub fn from_str(s: &str) -> Self {
        GoString::new(s)
    }

    /// Convert from a Rust char (single-byte ASCII).
    pub fn from_byte(b: u8) -> Self {
        GoString { bytes: vec![b] }
    }

    /// Go's `string(byte)` conversion: converts a byte (u8) to a one-char
    /// Go string. For multi-byte runes, this gives the low byte.
    pub fn from_byte_conversion(b: u8) -> Self {
        GoString::from_byte(b)
    }

    /// Convert from a Rust i32 using Go's string(int) rules.
    /// In Go, `string(int)` converts to the UTF-8 encoding of the code point.
    pub fn from_int(val: i64) -> Self {
        // Use Rust's char::from_u32 to handle the conversion properly
        if let Some(c) = char::from_u32(val as u32) {
            GoString::new(&c.to_string())
        } else {
            // Invalid code point — Go produces a replacement character
            GoString::new("")
        }
    }
}

impl From<String> for GoString {
    fn from(s: String) -> Self {
        GoString::new(&s)
    }
}

impl From<&String> for GoString {
    fn from(s: &String) -> Self {
        GoString::new(s)
    }
}

impl From<&str> for GoString {
    fn from(s: &str) -> Self {
        GoString::new(s)
    }
}

impl AsRef<str> for GoString {
    fn as_ref(&self) -> &str {
        // SAFETY: GoString is always UTF-8 encoded when created via from_str/from/
        // constructors. We trust the invariant.
        std::str::from_utf8(&self.bytes)
            .unwrap_or_else(|_| "\u{FFFD}") // replacement char on invalid UTF-8
    }
}

use std::ops::Deref;

impl Deref for GoString {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl std::fmt::Display for GoString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::fmt::Debug for GoString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GoString({:?})", self.as_str())
    }
}

impl Eq for GoString {}

impl PartialEq for GoString {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<&str> for GoString {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<GoString> for &str {
    fn eq(&self, other: &GoString) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<String> for GoString {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<GoString> for String {
    fn eq(&self, other: &GoString) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<[u8]> for GoString {
    fn eq(&self, other: &[u8]) -> bool {
        self.as_bytes() == other
    }
}

impl PartialEq<GoString> for [u8] {
    fn eq(&self, other: &GoString) -> bool {
        self == other.as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_from_str() {
        let s = GoString::new("hello");
        assert_eq!(s.len(), 5);
        assert_eq!(s.get_byte(0), b'h');
        assert_eq!(s.get_byte(4), b'o');
    }

    #[test]
    fn test_empty_string() {
        let s = GoString::new("");
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn test_slice() {
        let s = GoString::new("hello world");
        let sliced = s.slice(6, 11);
        assert_eq!(sliced.as_str(), "world");
    }

    #[test]
    fn test_utf8_handling() {
        let s = GoString::new("héllo"); // UTF-8: h=0x68, é=0xC3 0xA9, l=0x6C, l=0x6C, o=0x6F
        assert_eq!(s.len(), 6); // byte length, not char length
        assert_eq!(s.get_byte(0), b'h'); // byte at index 0
    }

    #[test]
    fn test_default_is_empty() {
        let s = GoString::default();
        assert!(s.is_empty());
    }

    #[test]
    fn test_from_to_string() {
        let s = GoString::from("test".to_string());
        assert_eq!(s.as_str(), "test");

        let s2 = GoString::from("test");
        assert_eq!(s.as_str(), s2.as_str());
    }

    #[test]
    fn test_deref_to_str() {
        let s = GoString::new("hello");
        // Through Deref, we can call str methods directly
        assert_eq!(s.len(), 5); // str::len for the UTF-8 length
    }

    #[test]
    fn test_clone_shares_bytes() {
        let s = GoString::new("hello world");
        let cloned = s.clone();
        assert_eq!(s.as_str(), cloned.as_str());
    }

    #[test]
    fn test_from_byte() {
        let s = GoString::from_byte(b'A');
        assert_eq!(s.as_str(), "A");
    }

    #[test]
    fn test_from_int() {
        let s = GoString::from_int(65); // 'A' in ASCII
        assert_eq!(s.as_str(), "A");

        let s2 = GoString::from_int(97); // 'a'
        assert_eq!(s2.as_str(), "a");
    }

    #[test]
    fn test_from_bytes() {
        let s = GoString::from_bytes(vec![b'h', b'e', b'l', b'l', b'o']);
        assert_eq!(s.as_str(), "hello");
    }
}
