# RFC 003: Go Multi-Return Values

**Status**: NOT YET IMPLEMENTED
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

1. **Parameter and return type parsing** — Currently,
   `GoFnOutput::parse` already parses multiple types
   (in a tuple branch), but there is no support for handling
   Go's multi-return syntax: `(int, error)` → `(i32, &str)`
   separately.
2. **Body transpilation for multi-statement returns**
    — Go functions like `(n / d, n % d)` are already
    translated into a Rust tuple. The transpiler already does this.
3. **Edge cases** — Mixed tuple types, e.g. `(int, string)`,
    should map to `(i32, String)`.
4. **Go's `v, err := foo()` idiom**
    — If one or more returns is optional (e.g. the
    first return value handles an error or condition),
    there is no current support for this.

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