# 🍂 Gourd — Roadmap

> Write Go. Get Rust. At compile time.

Gourd transpiles **basic Go syntax** into Rust at compile time via a procedural macro. It supports a subset of Go's syntax surface — type names, builtins, control flow, struct definitions, closures, and `defer`. It does **not** yet support virtually any idiomatic Go pattern outside of algorithmic exercises.

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
| `copy` | ✅ `std_copy` in prelude |
| `delete` | ✅ `std_delete` in prelude |
| `defer` | ✅ Inline Drop guard generation |
| `recover` | ✅ | Thread-local panic slot + `go_with_panic_slot` wrapper for `catch_unwind` integration.
| `complex` | ✅ | Construction, arithmetic (+/-/*//), real/imag extraction all implemented. |
| `min` / `max` | ✅ `min(a, b)`, `max(a, b)` with `<T: PartialOrd>` |

### Operators

All arithmetic, unary, and comparison operators transpiled. Pointer operators `&` (address-of) and `*` (dereference) also handled.

### Variadic parameters

Go variadic parameters (`func f(args ...int)`) are mapped to Rust slice references (`&[i32]`).

### Literals

Numeric, string, bool, slice literals, map literals, struct literals, ranges.

### Concurrency

Real `crossbeam`-backed primitives: `GoScheduler`, `GoChannel`, `GoSelect`, `SchedulerMap`, `GoFuture`.

---

## Closures

Closure parsing is supported in the transpiler:

| Go | Rust | Status |
|----|------|--------|
| `func() { body }` | `|| { body }` | ✅ |
| `func(x int) int { body }` | `|x: i32| -> i32 { body }` | ✅ |
| `func(arr []int) int { body }` | `|arr: &[i32]| -> i32 { body }` | ✅ |
| `func() (a, b int) { body }` | `|| -> (i32, i32) { body }` | ✅ |
| `if` in closure body | `if` in Rust closure | ✅ (as fallback) |
| `len()`, `[]` in closure body | ✅ | Transpiled via HIR path — `len(arr)` → `arr.len() as i32`, `arr[i]` → `arr[i as usize]` |

## Standard Library Mappings

| Go Package | Functions | Status |
|------------|-----------|--------|
| `strings` | Replace, ReplaceAll, HasPrefix, HasSuffix, Contains, Split, Join, Index, LastIndex, Trim, TrimLeft, TrimRight, ToUpper, ToLower, Repeat, Fields | ✅ 16 functions |
| `os` | Open, ReadFile, WriteFile, Mkdir, MkdirAll, Remove, Chdir, Getenv, Setenv, EnvKeys, Args | ✅ 11 functions |
| `io` | Copy, ReadAll | ✅ 2 functions |
| `bytes` | Contains, HasPrefix, HasSuffix, Index, Split, Join, Replace | ✅ 7 functions |
| `encoding/json` (`json`) | Marshal, Unmarshal | ✅ 2 functions |
| `time` | Now, Since, Until, Sleep | ✅ 4 functions |
| `math` | Abs, Sqrt, Floor, Ceil, Round, Min, Max, PI, E, Exp, Log, Log10, Pow, Sign | ✅ 14 functions |
| `byte` | Of, RuneOf, StringToBytes, BytesToString | ✅ 4 functions |

### Package emulation (`gourd::packages::*`)

Package emulation code lives in `gourd/src/packages/`:
- `os_ops.rs` — 11 os functions
- `strings_ops.rs` / `strings.rs` — 16 strings functions
- `json_ops.rs` — 2 json functions
- `io_ops.rs` — 2 io functions
- `bytes_ops.rs` — 7 bytes functions
- `math_ops.rs` — 14 math functions
- `byte_ops.rs` — 4 byte utilities
- `time_ops.rs` — 4 time functions

### New stdlib: copy, delete, append

These three Go builtin functions are now implemented as standard library functions:

| Go | Rust (transpiled) | Runtime |
|----|-------------------|---------|
| `copy(dst, src)` | `::gourd::prelude::std_copy(&mut dst, &src)` | `std_copy<T: Clone>(dst: &mut [T], src: &[T]) -> i32` |
| `delete(m, key)` | `::gourd::prelude::std_delete(m, key)` | `std_delete<T, V>(map: HashMap<T, V>, key: T) -> Option<V>` |
| `append(slice, items...)` | `::gourd::prelude::std_append(slice, &[items...])` | `std_append<T: Clone>(slice: Vec<T>, items: &[T]) -> Vec<T>` |

## Partially Implemented (tests not passing)

| Go Pattern | Status | Issue |
|------------|--------|-------|
| **Closure builtins** | ✅ | `len()`, `[]` indexing inside closure bodies fully transpiled via HIR path.

---

## Missing Features

### What won't work (and why it matters)

| Go Pattern | Status | Impact |
|------------|--------|--------|
| **Closures** `func() { ... }` | ✅ | Argument forwarding, captures, nested closures, and body builtins (`len()`, `[]` indexing).
| **defer** `defer cleanup()` | ✅ | Parsed → Drop guard; no dedicated tests yet |
| **Error handling** `if err != nil` | ✅ | Transpiles to `if let Result::Err(err) = expr` |
| **Pointers** | ✅ | `&` (address-of) and `*` (dereference) |
| **fmt builtins** | ✅ | `Sprintf/Print/Println/Printf` → format helpers |
| **Map params** | ✅ | `map[string]int` → `HashMap<String, i32>` |
| **switch** | ✅ | Both selector and no-selector forms |
| **Variadic params** `func f(...int)` | ✅ | Mapped to `&[T]` slice references |
| **Standard library calls** | ✅ | `strings`, `os`, `io`, `bytes`, `json`, `time`, `math`, `byte`, `fmt`, `std::copy`, `std::delete`, `std::append` |
| **min / max** | ✅ | `min(a, b)`, `max(a, b)` with `<T: PartialOrd>` |

### Still not implemented

| Go Pattern | Status | Impact |
|------------|--------|--------|
| **recover** `recover()` | ✅ | Parsed via conversion, emits `::gourd::prelude::recover()`. Runtime: thread-local slot + `go_with_panic_slot` wrapper. |
| **complex** number types | ✅ | Full runtime: `Complex64`/`Complex128` with Add, Sub, Mul, Div, conjugate, abs. |
| **for** without `range` | ✅ | `GoFor` + `ForLoop` variant with init/cond/post parsing and codegen. |
| **nil** comparison | ✅ | `NilComparison` variant with `.is_empty()` for maps, `.is_none()` fallback. |
| **Slice ranges** `text[start:end]` | ✅ | `Slice { collection, start, end }` variant with codegen. |
| **var declarations** | ✅ | `VarDecl { name, type_hint }` statement with zero-value initialization. |
| **Closure builtins** | ✅ | `len()`, `[]` indexing inside closure bodies — fully working via HIR path. `len(arr)` → `arr.len() as i32`, `arr[i]` → `arr[i as usize]`. |

---

## Status

| Metric | Value |
|--------|-------|
| **Real-world Go coverage** | ~5–8% |

### Debugging

Set `GOURD_DEBUG=1` to enable verbose diagnostic output during transpilation:

```bash
GOURD_DEBUG=1 gourd transpile "func hello() int { return 42 }"
```

The transpiler prints parsing details, type mappings, and transpilation steps to stderr. Useful for investigating failed transpilation or unexpected output. Zero overhead when unset.

### What would it take to be viable?

1. **Full closure support** — argument forwarding, captures, nested closures
2. **Standard library mapping** — `net/http`, `database/sql`, `sync`, `reflect`, `rand` → Rust std
3. **Generics** — needed for type-safe collections
4. **C-style `for` loops** — `for i := 0; i < n; i++`

Without all four: probably a toy. With all four: maybe 40–50% coverage — useful for algorithmic and CLI code.
