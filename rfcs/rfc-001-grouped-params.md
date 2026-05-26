# RFC 001: Go-style Parameter Shorthand (grouped params)

**Status**: ✅ IMPLEMENTED
**Priority**: 1 (Highest)
**Complexity**: Medium

## Goal

Transform Go syntax `func foo(a, b, c int)` → Rust `fn foo(a: i32, b: i32, c: i32)`.

```go
go! {
    func foo(a, b, c int) string {
        a + b + c
    }
}
// Transpiles to:
fn foo(a: i32, b: i32, c: i32) -> String {
    a + b + c
}
```

## Implementation

- `GoFnInputs::Parse` uses fork-based lookahead to distinguish group-comma from parameter-separator. When the identifier following a comma is a known Go type keyword, the comma is rolled back and treated as a param separator.
- Handled in `gourd-codegen/src/transpiler.rs` (`GoFnInputs::parse`).
