# Gourd

Transpiles inline Go expressions into valid Rust via a procedural macro at compile time.

# ROADMAP

This document outlines remaining features to add for Go → Rust compatibility.

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

### `string(bytes_slice)` conversions
`string(bytes_slice)` → `std::str::from_utf8(bytes_slice).unwrap()`

### Type conversions
Go's explicit type conversions `(int)(float_val)` → Rust casts.

### Slice literals
`[]int{1, 2, 3}` — array literal syntax (`transpile_array` works but Go slice literals are a separate concern).

### `select` statement

### `defer`

### `panic` / `recover`

### Goroutines (`go func { }()`)

### Channels (`ch <- value`, `<- ch`)

### `sync` primitives

### Map literals `map[string]int{"a": 1}`

### `for k, v := range slice` with init

### Type assertions and type switches

### Go interface → trait mapping

### `struct{}` bare structs (without fields)

### `assert` statements

### Function declarations inside blocks

### `continue` statement

### Labels (Go labeled break/continue within control flow structure)

## FEATURE ROADMAP (remaining, by priority)

### 1. Go-style `string()` conversions

**Status:** NOT YET IMPLEMENTED

**Goal:** Support Go implicit byte/byte slice string conversions:

```go
go_len(some_bytes) → std::str::from_utf8(some_bytes).unwrap()
```

**Effort:** Medium (must parse Go `string()` function — and emit call converted to a `call` parser)
**Value:** Medium (common Go pattern)

### 2. Receiver functions (method declarations)

**Status:** NOT YET IMPLEMENTED

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

**Effort:** Medium (requires an `impl` block in the destination, special handling of receiver types (`&&mut Foo → &Foo))
**Value:** High (common Go pattern)

### 3. Interfaces

**Status:** NOT YET IMPLEMENTED

**Goal:** Go interfaces:

```go
interface {
    HelpMe(a int, b int) int
}
// Transpiles to: trait with generics.
```

**Value:** Medium (many Go programs classify types)

### 4. Multi-return values

**Status:** NOT YET IMPLEMENTED (RFC 003)

```go
go! {
    func divmod(n int, d int) (int, int) {
        (n / d, n % d)
    }
}
```

### 5. `for i, v := range mySlice` with init.

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

### 6. Channels

**Status:** NOT YET IMPLMENTED.

```go
ch <- value
x := <- ch
```

**Effort:** Very high (must create async and sync synchronization primitives)
**Value:** Low (Rust pattern for channels are different anyhow, `Channel`
            (anowned singly-crning handles)

### 7. Goroutines (`go func { ... }()`)

**Status:** NOT YET IMPLEMENTED.

```go
go foo(42)
```

**Effort:** Very high (must spawn bare-metal thread, or async task)
**Value:** Low (Rust handles concurrency very differently)

### 8. Error handling

**See: [RFC 004: Go Error Handling → Rust Result](../rfcs/rfc-004-error-handling.md)**

**Status:** Planning — `error` maps to `Box<dyn std::error::Error>`. Body-level
`Ok(val)` / `Err(err)` wrapping deferred.

### 9. Struct field assignment

**Status:** NOT YET IMPLEMENTED (already guarded in `transpile_let`, working via `Expr::Let` dispatch).
**Value:** Would surface when struct {} declared)

### 10. Map and Slice literals

**Status:** NOT YET IMPLEMENTED.

```go
v := []int{1, 2, 3}
m := map[string]int{"a": 1, "b": 2}
```

**Effort:** Medium
**Value:** High (common Go idiom)
