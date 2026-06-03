# 🍂 Gourd — Roadmap

> Write Go. Get Rust. At compile time.

Gourd transpiles **basic Go syntax** into Rust at compile time via a procedural macro. It supports a subset of Go's syntax surface — type names, builtins, control flow, struct definitions. It does **not** yet support standard library calls, closures, or virtually any idiomatic Go pattern outside of algorithmic exercises.

---

## Implemented Features

### Function Declarations

| Go | Rust | Notes |
|----|------|-------|
| `func foo(a int, b int) int { ... }` | `fn foo(a: i32, b: i32) -> i32 { ... }` | |
| `func foo(a, b, c int) int { ... }` | `fn foo(a: i32, b: i32, c: i32) -> i32 { ... }` | Parameter grouping |
| `func (f Foo) Method(z int) int { ... }` | `impl Foo { fn Method(&self, z: i32) -> i32 { ... } }` | Value receiver |
| `func (f *Foo) Method(z int) int { ... }` | `impl Foo { fn Method(&mut self, z: i32) -> i32 { ... } }` | Pointer receiver |
| `return a, b` | `return (a, b)` | Multi-return |

Name preservation: Go camelCase names stay camelCase. `clippy` warnings suppressed.

### Struct & Interface Definitions

| Go | Rust | Notes |
|----|------|-------|
| `struct Foo { x int, y int }` | `struct Foo { pub x: i32, pub y: i32 }` | Fields auto-`pub` |
| `interface Shape { Name() string }` | `trait Shape { fn name(&self) -> String; }` | |

### Types

| Go | Rust |
|----|------|
| `int`, `int8`–`int64` | `i8`–`i64` |
| `uint`, `uint8`–`uint64` | `u8`–`u64` |
| `uintptr` | `usize` |
| `byte` | `u8` |
| `rune` | `char` |
| `float32`, `float64` | `f32`, `f64` |
| `string` | `String` |
| `bool` | `bool` |
| `error` | `Box<dyn std::error::Error>` |
| `[]T` (slice type) | `&[T]` |
| `chan T` | `GoChannel::<T>::new()` |

### Control Flow

| Go | Rust |
|----|------|
| `if / else / else if` | `if / else / else if` |
| `switch n { case 1: ... }` | `match n { 1 => ... }` |
| `switch { case ok: ... }` | `if / else if chain` |
| `for i, v := range data` | `for (i, v) in data.iter().copied().enumerate()` |
| `for i := range data` | `for i in 0..data.len()` |
| `while` | `while` |
| `break`, `continue` | `break`, `continue` |

### Builtins

| Builtin | Status |
|---------|--------|
| `len(s)`, `cap(s)` | ✅ Slices only |
| `string(bytes)` | ✅ `[]byte` → `String` |
| `int(x)`, `bool(x)`, etc. | ✅ Type conversions |
| `make(chan/map/slice)` | ✅ All three types |
| `new(Foo)` | ✅ `Foo::default()` |
| `panic("msg")` | ✅ `panic!("msg")` |
| `append(slice, items)` | ✅ Push to Vec copy |
| `x.(T)` (type assertion) | ✅ Cast/downcast |
| `recover` | ❌ |
| `defer` | ❌ |
| `complex` | ❌ |
| `min` / `max` | ❌ |

### Operators

All arithmetic, unary, and comparison operators transpiled.

### Literals

Numeric, string, bool, slice literals, map literals, struct literals, ranges.

### Concurrency

Real `crossbeam`-backed primitives: `GoScheduler`, `GoChannel`, `GoSelect`, `SchedulerMap`, `GoFuture`.

---

## Partially Implemented (tests not passing)

| Go Pattern | Status | Issue |
|------------|--------|-------|
| **Closures** `func() { ... }` | ⚠️ | Partial implementation; `go_to_rust_closure` not properly exposed; import errors in tests |
| **`for` range** | ⚠️ | `range` keyword sometimes parsed as identifier, not loop construct |
| **`channel_ops`** | ⚠️ | `<` comparison on `GoChannel<i32>` — type doesn't implement `PartialOrd` |
| **`continue`** | ⚠️ | Runtime assertion failure in test |
| **`struct_literals`** | ⚠️ | Breaks on closure fallback path |
| **`switch_extended`** | ⚠️ | Breaks on closure fallback path |
| **`transpile_go_fn`** | ⚠️ | Breaks on closure fallback path |
| **`type_assertion`** | ⚠️ | Breaks on closure fallback path |

---

## Missing Features

### What won't work (and why it matters)

| Go Pattern | Status | Impact |
|------------|--------|--------|
| **Closures** `func() { ... }` | ⚠️ | Partial; not working in tests — no higher-order functions, no sorting |
| **defer** `defer cleanup()` | ❌ | No RAII pattern |
| **Error handling** `if err != nil` | ❌ | Dominant Go error handling pattern |
| **recover** `recover()` | ❌ | |
| **Variadic params** `func f(...int)` | ❌ | Most stdlib functions are variadic |
| **Pointers in expressions** `&x`, `*p` | ❌ | Can't dereference or take addresses |
| **Standard library calls** | ❌ | No `fmt`, `os`, `io`, `sort`, `strings` |

---

## Status

| Metric | Value |
|--------|-------|
| **Real-world Go coverage** | ~5% |
| **syn::Expr variants covered** | 26 of ~39 |
| **Builtins implemented** | 9 of ~14 |
| **Test code** | ~40% commented-out TODO stubs |

### Working tests (passing)

| Test file | Result |
|-----------|--------|
| `append_builtin.rs` | ✅ 4/4 |
| `go_fn.rs` | ✅ 9/9 |
| `interface_tests.rs` | ✅ 7/7 |
| `make_builtin.rs` | ✅ 5/5 |
| `multi_case_switch.rs` | ✅ 1/1 |
| `new_builtin.rs` | ✅ 4/4 |
| `panic_builtin.rs` | ✅ 4/4 |
| `receiver_tests.rs` | Compiles (0 tests) |
| `shorthand_query.rs` | Compiles |

### What would it take to be viable?

1. **Closures** — the single biggest gap; enables sorting, callbacks, etc.
2. **`append` / `copy` / `delete`** — `append` works, `copy`/`delete` don't
3. **`defer`** — critical for resource management
4. **Error handling** — `if err != nil` is the dominant Go pattern
5. **Standard library mapping** — even `fmt → println!`, `math → std::f64` moves the needle
6. **Generics** — needed for type-safe collections

Without all six: probably a toy. With all six: maybe 30–40% coverage — useful for algorithmic code.
