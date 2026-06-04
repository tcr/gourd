//! Tests for Go standard library package function transpilation.
//!
//! These test the transpiler's ability to generate calls to the package
//! emulation functions via `::gourd::prelude::*`.

use gourd_macro::go;

// ─── Strings operations ───────────────────────────────────────────────────

// Test: strings.Replace
go! {
    func goReplace() string {
        return strings.Replace("hello world world", "world", "rust", 2)
    }
}

// Test: strings.ReplaceAll
go! {
    func goReplaceAll() string {
        return strings.ReplaceAll("aaa", "a", "b")
    }
}

// Test: strings.HasPrefix
go! {
    func goHasPrefix() bool {
        return strings.HasPrefix("hello world", "hello")
    }
}

// Test: strings.HasSuffix
go! {
    func goHasSuffix() bool {
        return strings.HasSuffix("hello world", "world")
    }
}

// Test: strings.Contains
go! {
    func goContains() bool {
        return strings.Contains("hello world", "world")
    }
}

// Test: strings.Join
go! {
    func goJoin() string {
        return strings.Join(["a", "b", "c"], ",")
    }
}

// Test: strings.Split
go! {
    func goSplit() []string {
        return strings.Split("a,b,c", ",")
    }
}

// Test: strings.Index
go! {
    func goIndex() int {
        return strings.Index("hello world", "world")
    }
}

// Test: strings.Trim
go! {
    func goTrim() string {
        return strings.Trim("  hello  ", " ")
    }
}

// Test: strings.ToUpper
go! {
    func goToUpper() string {
        return strings.ToUpper("hello")
    }
}

// ─── Time operations ──────────────────────────────────────────────────────

// Test: time.Now
go! {
    func goTimeNow() int64 {
        return time.Now()
    }
}

// Tests

#[test]
fn test_strings_replace() {
    assert_eq!(goReplace(), "hello rust rust");
}

#[test]
fn test_strings_replace_all() {
    assert_eq!(goReplaceAll(), "bbb");
}

#[test]
fn test_strings_has_prefix() {
    assert_eq!(goHasPrefix(), true);
}

#[test]
fn test_strings_has_suffix() {
    assert_eq!(goHasSuffix(), true);
}

#[test]
fn test_strings_contains() {
    assert_eq!(goContains(), true);
}

#[test]
fn test_strings_join() {
    assert_eq!(goJoin(), "a,b,c");
}

#[test]
fn test_strings_split() {
    let result = goSplit();
    assert_eq!(result, vec!["a", "b", "c"]);
}

#[test]
fn test_strings_index() {
    assert_eq!(goIndex(), 6);
}

#[test]
fn test_strings_trim() {
    assert_eq!(goTrim(), "hello");
}

#[test]
fn test_strings_to_upper() {
    assert_eq!(goToUpper(), "HELLO");
}

#[test]
fn test_time_now() {
    let t = goTimeNow();
    assert!(t >= 0);
}
