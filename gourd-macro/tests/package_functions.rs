//! Tests for Go standard library package function transpilation.
//!
//! Each `go!` block groups an `import` statement with its related test
//! functions, demonstrating how `import` generates targeted `use` imports
//! scoped to the functions that need them.
//!
//! ```go
//! import strings  // → use gourd::packages::strings::*;
//! import time     // → use gourd::packages::time::*;
//! import os       // → use gourd::packages::os::*;
//! ```

use gourd_macro::go;

// ─── Strings package: import + test functions ─────────────────────────────

go! {
    import strings

    func goReplace() string {
        return strings.Replace("hello world world", "world", "rust", 2)
    }

    func goReplaceAll() string {
        return strings.ReplaceAll("aaa", "a", "b")
    }

    func goHasPrefix() bool {
        return strings.HasPrefix("hello world", "hello")
    }

    func goHasSuffix() bool {
        return strings.HasSuffix("hello world", "world")
    }

    func goContains() bool {
        return strings.Contains("hello world", "world")
    }

    func goJoin() string {
        return strings.Join(["a", "b", "c"], ",")
    }

    func goSplit() []string {
        return strings.Split("a,b,c", ",")
    }

    func goIndex() int {
        return strings.Index("hello world", "world")
    }

    func goTrim() string {
        return strings.Trim("  hello  ", " ")
    }

    func goToUpper() string {
        return strings.ToUpper("hello")
    }
}

// ─── Time package: import + test functions ────────────────────────────────

go! {
    import time

    func goTimeNow() int64 {
        return time.Now()
    }
}

// ─── OS package: import only (demo) ───────────────────────────────────────
// Note: os.Open expects &str but the transpiler generates String for literals.
// The import os syntax works; os function compatibility is a separate concern.

go! {
    import os
}

// ─── Tests ────────────────────────────────────────────────────────────────

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
