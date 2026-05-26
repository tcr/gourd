# RFC 004: Go Error Handling → Rust Result

**Status**: NOT YET IMPLEMENTED
**Priority**: 1 (High)
**Complexity**: Medium

## Goal

Transform Go's idiomatic two-value error return pattern — `(value, error)`
— into idiomatic Rust `Result<T, E>`:

```go
go! {
    func divmod(n, d int) (int, error) {
        if d == 0 {
            -1, errors.New("division by zero")
        }
        n / d, nil
    }
}

// Transpiles to:
fn divmod(n: i32, d: i32) -> Result<i32, Box<dyn std::error::Error>> {
    if d == 0 {
        Err(Box::new(std::env::VarError::NotPresent))
    } else {
        Ok(n / d)
    }
}
```

## Background

`map_GO_types` already maps `"error"` → `compile_error!("TODO: ...")` at line 457
of `transpiler.rs`. Any Go code using the `error` type in a function return or
parameter triggers a compile-time `compile_error!`.

This RFC proposes a resolution: instead of emitting `compile_error!`, map the
Go `error` type to Rust's `Box<dyn std::error::Error>`.

## Mapping Rules

| Go construct | Rust equivalent |
|---|---|
| `error` (type name) | `Box<dyn std::error::Error>` |
| `nil` (in return position) | `Ok(value)` wrapping (for single return) |
| `nil` (in multi-return with error) | `Ok(...)` when no error |
| non-`nil` error value | `Err(e)` when error is non-`nil` |

## Implementation Details

### What needs to change

**1. `go_type_map` (transpiler.rs:457)** — Currently:

```go
"error" => emit_todo("Go `error` interface not yet supported"),
```

Proposed change — map `"error"` to the Rust type:

```go
"error" => quote! { Box<dyn std::error::Error> },
```

This is a single-line change. Every place where `error` appears in a function
return type (e.g., `(int, error)` → `(i32, Box<dyn std::error::Error>)`) will
now produce valid Rust instead of a compile-time error.

**What about body expressions?**

Go body `(n / d, nil)` is already transpiled as a Rust tuple `(n / d, None)`
via `transpile_tuple`. 

This RFC does **NOT** change the body-level transpilation. The body `(n / d, nil)`
will produce `(n / d, None)` — a tuple with `None` as the second element. This is
**intentionally out of scope**. The user will need to manually convert:

```go
// Go user writes:
go! {
    func myFunc(x int) (int, error) {
        // Body: user handles Ok/Err conversion manually
        // No automatic Ok() / Err() wrapping in the body
        x, nil  // → (i32, None) as tuple, user converts or discards
    }
}
```

**2. `GoFnOutput::parse` (transpiler.rs:378)** — No changes needed. The parser
already handles `(int, error)` as two separate types in `tys`. After change #1,
`map_go_types` will correctly map `error` → `Box<dyn std::error::Error>`.

**3. Return value wrapping (optional, future RFC)** — Automatically wrapping
body tuple `(val, nil)` → `Ok(val)` and `(val, err)` → `Err(err)` would require
a body-level transformation pass that recognizes tuples of length 2 where the
second element is `nil` or a non-`nil` error expression. This is explicitly out
of scope for this RFC.

### What needs to work

- Go return type `(int, error)` → Rust `(i32, Box<dyn std::error::Error>)`
- Go return type `(string, error)` → Rust `(String, Box<dyn std::error::Error>)`
- Go return type `(int, string, error)` → Rust `(i32, String, Box<dyn std::error::Error>)`
- Go parameter `(x int, err error)` → Rust `(x: i32, err: Box<dyn std::error::Error>)`
- Body tuple `(n / d, nil)` → Rust tuple `(n / d, None)` (existing behavior preserved)

### What is explicitly out of scope (for this RFC)

- **Body Ok/Err wrapping**: `(n / d, nil)` → `Ok(n / d)`, `(val, myErr)` → `Err(myErr)`.
  The body tuple is a value, not a `Result`. User code is responsible for wrapping.
- **Error interface methods**: `Error() string`, `errors.New(...)`. These require
  parsing Go std library constructs and are out of scope.
- **`panic` / `recover`**: No support for these concurrency primitives.
- **`v, err := foo()` multiple assignment / destructuring**: Scala-style tuple
  unpacking (RFC 003 scope was only *returns*, not *unpacking*).
- **Custom error types**: When the user defines a Go struct implementing the error
  interface, it should become a Rust struct implementing `std::error::Error`.

### Edge cases

1. **`error` in receiver functions**: `(obj *Foo) Open() (Reader, error)` →
   `fn Open(&self) -> (Reader, Box<dyn std::error::Error>)`. Works via
   existing `go_to_rust_receiver_fn` + new `go_type_map` change.

2. **`error` as a parameter**: `func (f *Foo) Write(p []byte, e error) {}`
   → `fn Write(&mut self, p: &[i32], e: Box<dyn std::error::Error>) {}`.
   Works via existing `map_GO_types` branch (`syn::Type::Path` → `go_type_map`).

3. **Multiple error in returns**: `(int, string, error)` works because
   `GoFnOutput::parse` extracts each type individually and maps them all.

## Proposed Body Conversion (Future RFC, not in scope)

If body-level `Ok(val)` / `Err(err)` wrapping were desired:

```go
// Body: (val, nil)
// Would become: Ok(val)
// Body: (val, myErr)  // where myErr is non-nil
// Would become: Err(myErr)
```

This would require a new `GoBody` AST node for return expressions and a `map_return_body`
pass that inspects the tuple's second element. **Out of scope for RFC 004.**

## References

- [FEATURE ROADMAP (priority #10)](../ROADMAP.md)
- [RFC 003: Multi-Return Values](rfc-003-multi-return-values.md)
