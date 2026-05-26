# RFC 003: Go Multi-Return Values

**Status**: ✅ IMPLEMENTED
**Priority**: 3 (Medium)
**Complexity**: Low

## Goal

Transform Go's idiomatic practice of returning multiple values
from a single function into idiomatic Rust tuple returns,
such as:

```go
go! {
    func divmod(n, d int) (int, int) {
        (n / d, n % d)
    }
}

// Transpiles to:
fn divmod(n: i32, d: i32) -> (i32, i32) {
    (n / d, n % d)
}
```

## Background

This is already partially handled by `map_GO_types` which
remaps `(int, int)` → `(i32, i32)` when it sees a `Type::Tuple`.
The tuple type mapping is in place, but no special
handling for Go's multi-return value syntax exists yet.

## Implementation Details

### What needs to change

This feature required **zero code changes** — all the infrastructure was already
in place from previous implementations:

1. **`GoFnOutput::parse`** (transpiler.rs:378) — Already parses comma-separated
   return types, extracting each as a `syn::Type` into a `Vec<syn::Type>`.
2. **`map_go_types`** (transpiler.rs:487) — Already handles `Type::Tuple`
   by recursively mapping each element type through `map_go_types`.
3. **`go_to_rust_fn`** (transpiler.rs:511) — Already outputs `-> (T1, T2, ...)`
   when `output.tys.len() > 1`.
4. **`transpile_tuple`** (transpiler.rs:107) — Already translates Go tuple
   expressions `(a, b)` → Rust `(a, b)` by mapping each element.

### Edge cases verified by tests

- `(int, int)` → `(i32, i32)` (via `map_go_types` on individual `Type::Path`)
- `(int, string)` → `(i32, String)` (mixed tuple types)
- `(int, int, string)` → `(i32, i32, String)` (triple multi-returns)
- Body expressions `(n / d, n % d)` → Rust `(n / d, n % d)` (via `transpile_tuple`)

### What needs to work

- Go `(n / d, n % d)` → Rust `(n / d, n % d)`
- Go return types `(int, string)` → Rust `(i32, String)`
- Go return types `(int, int)` → Rust `(i32, i32)` (via `map_go_types`).
- Mixed types: `(int, string)` → `(i32, String)`

### What is explicitly out of scope (for this RFC)

- Go `v, err := foo()` — Go's multiple assignment / multi-returns.
  This is tuple de-assignment of tuples. (Future: support ignoring
  returns with `_`.)

## References

- [FEATURE ROADMAP (priority #6)](../ROADMAP.md)