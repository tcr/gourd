# Gourd

Transpiles inline Go expressions into valid Rust via a procedural macro at compile time.
See embedded tests for the currently supported feature set.

# ROADMAP

This document outlines the next features to add for Go → Rust compatibility.
Prioritised by effort/value to the user experience.

## Completed

- [x] **`go_expr!`**: Inline expression transpiler (literals, binary/unary operators,
      `len`/`cap`, if/else, index, method calls, field access, short declarations,
      parentheses, ranges, loops).
- [x] **`go!`**: Function declaration entry point (type name mapping for
      `int` / `bool` / etc.), return type mapping, parameter shaping.
- [x] **Go-style slice type shorthand** (`[]T` → `&[T]`): Slice detection and
      parameter generation, group-aware slicing in `[]T` (shorthand `a, b []int`
      → `a: &[i32], b: &[i32]`), `len()` returns `as i32` cast.
- [x] **Block transpilation**: Statement-level handling (expressions, locals,
      `let mut`, `break`, `return`). Tuple literals, cast expressions, and
      assignments—all transpiled inside function blocks.

## Current features (`go_expr!`)

| Category | What works today |
|----------|-----------------|
| Literals | `42`, `0xff`, `1e3`, `"hello"`, `true`/`false` |
| Paths | `nil` → `None`, `true`, `false` |
| Binary operators | `+ - * / % ^ & | << >> == != > >= < <=` |
| Unary operators | `-` `(negation)`, `!` `(not)`, `*` `(dereference)` |
| Calls | `len(x)`, `cap(x)`, generic function calls |
| Ranges | `0 .. 10`, `0 ..= 10` |
| If / If-else | `if cond { ... } else { ... }` |
| Indexing | `v[i]`, `m[k]`, `m[k1][k2]` |
| Loops | `loop`, `for`, `while` (infinite) |
| Method calls | `s.method(args)` |
| Field access | `pt.0`, `s.field` |
| Short decl | `x := y` → `let x = y` |
| Blocks | `{ stmt; expr }` (final expression is the value) |
| Tuple literals | `(a, b)` — multis |
| Cast expressions | `x as T` |
| Assignments | `x = y` (mutable assignment after `let mut`) |
| Return / Break | `return expr`, `break` |

## Undone (kept as `compile_error!` sinks)

### Subscripting / indexing
> `slice[i]`, `map[k]`, `string[i]`, `array[i]` — works by
> `transpile_index` but errors when the element type is unknown.

### String titurations
> `string(bytes_slice)` → `std::str::from_utf8(bytes_slice).unwrap()`

### Type conversions
> Go's explicit type conversions `(int)(float_val)` → Rust casts.

### String slicing
> `string(some_bytes)` or `string(byte_slice)` — implicit conversions
> are implicit in Go, explicit in Rust.

## Optional (perhaps out of scope for an experimental

---

### Go → Rust  next volumes: transpile each statement; the final expression
### becomes the block's value.

**TODO: handle the following statement types:**

- [ ] `Stmt::Break`
- [ ] `Stmt::Continue`
- [ ] `Stmt::If`
- [ ] `Stmt::ForLoop`
- [ ] `Stmt::While`
- [ ] `Stmt::Loop`
- [x] `Stmt::Expr`
- [x] `Stmt::Local`
- [x] `Stmt::Break` (via `Expr::Break`)
- [x] `Stmt::Return` (via `Expr::Return`)
- [ ] assertion statements
- [ ] function declarations (recursively handled — [TODO])
- [ ] struct declarations (recursively handled — [TODO])
- [ ] enum declarations (recursively handled — [TODO])
- [ ] impl blocks (recursively handled — [TODO])
- [ ] use statements (recursively handled — [TODO])
- [ ] static declarations (recursively handled — [TODO])
- [ ] const declarations (recursively handled — [TODO])
- [ ] type aliases (recursively handled — [TODO])

---

## FEATURE ROADMAP (by priority)

### 1. Go-style parameter shorthand

**Status: ✅ IMPLEMENTED**

**Goal:** Transform Go syntax `func foo(a, b, c int)` → Rust `fn foo(a: i32, b: i32, c: i32)`.

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

Also handles Go-style parameter grouping: `a, b int` → `a: i32, b: i32`,
including any number of grouped parameters (not limited to 2). The parser
uses fork-based lookahead: when the name after a group-comma is a known
Go type keyword (e.g. `int`, `string`), the comma is treated as a
parameter separator rather than a group-member, enabling correct parsing
for arbitrarily many grouped params (e.g. `a, b, c, d int`).

**Effort:** Medium (requires splitting one `syn::Pat` into several)
**Value:** High (common Go pattern, very easy to break missing)

### 2. Slice type shorthand

**Status: ✅ IMPLEMENTED**

**Goal:** Complex Go type `[]T` (slice → `&[T]` (reference slice syntax).

```go
go! {
    fn go_len(a []int) i32 {
        len(a)
    }
}
// Transpiles to:
fn go_len(a: &[i32]) -> i32 {
    a.len() as i32
}
```

Also supports Go-style parameter grouping: `a, b []int` → `a: &[i32], b: &[i32]`.

**Effort:** Low (edit `map_GO_types` to handle `[]T` type alias)
**Value:** Medium (common Go pattern)

### 3. Go ≠ Rust String metadata: bytes → string (`string()` builtin
**Status:** NOT YET IMPLEMENTED

**Goal:** Support Go implicit byte/byte slice string conversions:

```go
go_len(some_bytes) → std::str::from_utf8(some_bytes).unwrap()
```

**Effort:** Medium (must parse Go `string()` function — and emit call converted to a `call` parser)
**Value:** Medium (common Go pattern)

### 4. Go-generics: receiver functions

**Status:** NOT YET IMPLEMENTED

**Goal:** Go receiver methods:

```go
go! {
    func (f Foo) Bar(z int) int {
        f.x + z
    }
}
// Transpiles to:
impl Foo {
    fn Bar(&self, z: i32) -> i32 {
        self.x + z
    }
}
```

**Effort:** Medium (requires an `impl` block in the destination, special handling of receiver types (&&mut Foo → &Foo))
**Value:** High (common Go pattern)

### 5. Interfaces

**Status:** NOT YET IMPLEMENTED

**Goal:** Go interfaces:

```go
interface {
    HelpMe(a int, b int) int
}
// Transpiles to: trait with generics.
```

**Value:** Medium (many Go programs classify types)

### 6. Duplicate multi-func int, double-return values type:

**Status: ✅ IMPLEMENTED (RFC 003)**

```go
go! {
    func divmod(n int, d int) (int, int) {
        (n / d, n % d)
    }
}
```

Go-style `(int, int)` → Rust `(i32, i32)` via `map_go_types` on tuple body expressions.
Mixed tuple types: `(int, string)` → `(i32, String)` (verified by `test_fn_mixed_tuple_returns`).
Triple multi-returns: `(int, int, string)` → `(i32, i32, String)` (verified by `test_fn_triple_returns`).

### 7. IfInit: `for i, v := range mySlice` simulates.

**Status:** NOT YET IMPLEMENTED.

```go
go! {
    func my_func(a []int) {
        for i, v := range a {
            println(i, v)  // ← requires Go std library / orerror → Rust: `println!`
        }
    }
}
```

**Effort:** High
**Value:** High (common Go idiom, much less idiomatic in Rust)

### 8. Channels

**Status:** NOT YET IMPLMENTED.

```go
ch <- value
x := <- ch
```

**Effort:** Very high (must create async and sync synchronization primitives)
**Value:** Low (Rust pattern for channels are different anyhow, `Channel`
            (anowned singly-crning handles)

### 9. Goroutines (`go func { ... }()`)

**Status:** NOT YET IMPLEMENTED.

```go
go foo(42)
```

**Effort:** Very high (must spawn bare-metal thread, or async task)
**Value:** Low (Rust handles concurrency very differently)

### 10. For syntactic error handling patterns

**Status:** NOT YET IMPLEMENTED.

```go
v, err := foo()
if err != nil { handle(err) }
```

**Effort:** Medium (must handle multiple returns + `compile_error!` «TODO file error handling»)
**Value:** High (overused)

### 11. Struct field assignment

**Status:** NOT YET IMPLEMENTED (already guarded in `transpile_let`, working via `Expr::Let` dispatch).
**Value:** Would surface when struct {} declared)

### 12. Map and Slice literals

**Status:** NOT YET IMPLEMENTED.

```go
v := []int{1, 2, 3}
m := map[string]int{"a": 1, "b": 2}
```

**Effort:** Medium
**Value:** High (common Go idiom)
