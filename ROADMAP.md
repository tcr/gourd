# 🍂 Gourd — Roadmap

> Write Go. Get Rust. At compile time.

This document outlines the current state and remaining features for Go → Rust compatibility.

## Implemented Features

The following Go constructs are fully transpiled and tested. Each section notes
the actual test coverage vs. the documented claim — some items are documented
as supported but have many commented-out test stubs (`// NOTE: ... not yet`).

### Function Declarations

| Go | Rust | Notes |
|----|------|-------|
| `func foo(a int, b int) int { ... }` | `fn foo(a: i32, b: i32) -> i32 { ... }` | |
| `func foo(a, b, c int) int { ... }` | `fn foo(a: i32, b: i32, c: i32) -> i32 { ... }` | Parameter grouping |
| `func divmod(n, d int) (int, int) { ... }` | `fn divmod(n: i32, d: i32) -> (i32, i32) { ... }` | Multi-return |
| `func (f Foo) Method(z int) int { ... }` | `impl Foo { fn Method(&self, z: i32) -> i32 { ... } }` | Value receiver |
| `func (f *Foo) Method(z int) int { ... }` | `impl Foo { fn Method(&mut self, z: i32) -> i32 { ... } }` | Pointer receiver |
| **Name preservation**: Go camelCase names are preserved in Rust output (e.g., `goShorthand2`, `goAdd`). Rust's `clippy` snake_case convention warnings are suppressed via `#[allow(non_snake_case)]`. | | |

### Struct & Interface Definitions

| Go | Rust | Notes |
|----|------|-------|
| `struct Foo { x int, y int }` | `struct Foo { pub x: i32, pub y: i32 }` | Fields auto-`pub` |
| `interface Shape { Name() string }` | `trait Shape { fn name(&self) -> String; }` | Method names are snake_case from Go, struct names preserved |
| `interface Reader { Read(data []byte) []byte }` | `trait Reader { fn name(&self, data: &[u8]) -> Vec<u8>; }` | Parameter grouping + slice types |

> **Note on naming**: Go types (struct/interface names) are preserved as-is (camelCase). Rust trait names match the Go type name. Rust's `clippy` snake_case convention warnings for trait names and function names are suppressed with `#[allow(non_snake_case)]` so the Go naming style is preserved in the output.

### Types

| Go | Rust |
|----|------|
| `int`, `int8`, `int16` | `i8`, `i16` |
| `int`, `int32` | `i32` |
| `int64` | `i64` |
| `uint`, `uint8`, `uint16` | `u8`, `u16` |
| `uint`, `uint32` | `u32` |
| `uint64` | `u64` |
| `uintptr` | `usize` |
| `byte` | `u8` |
| `rune` | `char` |
| `float32` | `f32` |
| `float64` | `f64` |
| `string` | `String` |
| `bool` | `bool` |
| `error` | `Box<dyn std::error::Error>` |
| `[]T` (slice type) | `&[T]` |
| `chan T` | `GoChannel::<T>::new()` |

### Control Flow

| Go | Rust | Notes |
|----|------|-------|
| `if cond { ... } else { ... }` | `if cond { ... } else { ... }` | |
| `switch n { case 1: "one" default: "other" }` | `match n { 1 => "one", _ => "other" }` | |
| `switch { case ok: "ok" }` | `if cond { ... } else if ...` | No-selector switch |
| `for i, v := range data { ... }` | `for (i, v) in data.iter().copied().enumerate() { ... }` | |
| `for i := range data { ... }` | `for i in 0..data.len() { ... }` | Index-only |
| `while cond { ... }` | `while cond { ... }` | |
| `continue` | `continue` | |
| `break` | `break` | |

### Expressions & Builtins

| Go | Rust | Notes |
|----|------|-------|
| `len(s)` | `s.len() as i32` | | ⚠️ Partial (works for slices only, not maps/channels) |
| `cap(s)` | `s.len() as i32` | | ⚠️ Partial |
| `string(bytes)` | `string(bytes)` (imported from prelude) | `[]byte` → `String`; resolves via prelude import, not inline expansion |
| `x.(T)` | `x as T` (with type-specific coercion) | Type assertion |
| `return a, b` | `return (a, b)` | Multi-return |
| `x := y` | `let x = y` | Short declaration |
| `x = y` | `x = y` | Assignment |

> **Note on builtins**: Only `len`, `cap`, `string(bytes)`, type conversion calls
> (`int(x)`, `bool(x)`, etc.), and `make()` for channels have been implemented.
> Most other builtins emit `compile_error!` at compile time. See the Builtin
> Gap table below for the full list.

### Operators

| Go | Rust | Notes |
|----|------|-------|
| `+ - * / % ^ & \| << >>` | `+ - * / % ^ & | << >>` | Binary |
| `- ! *` (dereference) | `- ! *` | Unary |
| `== != > >= < <=` | `== != > >= < <=` | Comparison |

### Literals

| Go | Rust |
|----|------|
| `42`, `0xff`, `1e3`, `"hello"`, `true`/`false` | Direct Rust equivalents |
| `[]int{1, 2, 3}` | `vec![1, 2, 3]` | |
| `map[string]int{"a": 1}` | `HashMap::new(); m.insert("a", 1)` | | ⚠️ Fragile (raw string parsing for types) |
| `m[key]`, `m.get(key)` | `m[&key]`, `m.get(&key)` | Reference for map access |
| `Point{x: 1, y: 2}` | `Point { x: 1, y: 2 }` | Struct literals (named fields only) |

### Concurrency (crossbeam-backed)

| Go | Rust | Notes |
|----|------|-------|
| `go func() { ... }` | `GoScheduler::new().submit(|| { ... })` | Sequential goroutine simulation — see Runtime Concurrency Note below |
| `chan int` | `GoChannel::<i32>::new()` | Unbuffered channel |
| `chan int{10}` | `GoChannel::<i32>::with_capacity(10)` | Buffered channel |
| `ch <- value` | `ch.send(value)` | Send to channel |
| `return <-ch` | `return ch.recv().unwrap()` | Receive from channel |
| `select { case ... }` | `GoSelect::<T>::new().run()` | Channel select with default/timeout |

---

## `cargo test` — Verification Contract

When `gourd-check` is invoked as part of `cargo test` at the workspace root, the following contract must be maintained:

1. **Scoped verification**: `gourd-check` only scans `gourd/`, `gourd-codegen/`, and `gourd-codegen-core/` crates. No other packages (workspace members or dependencies) are included.
2. **Full coverage required**: Every `go! { ... }` macro form inside those scanned crates must have a corresponding `#[verify_rust_output({ ... })]` attribute. If a `go!` block lacks this attribute, `gourd-check` reports a test failure.

This ensures:
- Every Go construct transpiled through `go!` has a compile-time assertion verifying the output.
- No regression is possible — removing or changing `#[verify_rust_output]` coverage will cause `cargo test` to fail.
- New `go!` blocks cannot be committed without verification.

### Implementation approach

- `gourd-check` restricts file discovery to the three crates above (by path or workspace membership).
- After scanning for `go! { ... }` blocks, the validator checks each one for a `#[verify_rust_output]` attribute preceding it.
- Missing attributes are reported as test failures with file:line locations.

---

## Implemented Tools

| Tool | Purpose |
|------|---------|
| `go!` proc-macro | Transpiles inline Go → Rust at compile time |
| `#[verify_rust_output({ expected })]` | Compile-time assertion: transpiled output must match expected Rust |
| `gourd transpile` | CLI tool: transpile Go blocks from source files or stdin |
| `gourd-check` | Standalone validator: checks Go syntax via `go build`, Rust via `cargo check` |
| `cargo expand` | See expanded `go!` → Rust output in your crate |
| `validate_go()` / `validate_rust()` | Cross-language validation via temp dirs + real compilers |

---

## Runtime Primitives

The `gourd` crate provides runtime types for the concurrency simulation:

| Type | Purpose |
|------|---------|
| `GoScheduler` | Thread-safe task scheduler — `submit()`, `run()`, `clone()` | ⚠️ Sequential simulation — see Runtime Concurrency Note below (we love crossbeam & rayon!) |
| `GoChannel<T>` | Generic channel — `send()`, `recv()`, `try_send()`, `try_recv()`, `with_capacity()` | ⚠️ Single-threaded spin-wait; buffered channels in single-threaded contexts may hang |
| `GoSelect<T>` | Select with `send_case()`, `recv_case()`, `with_default()`, `with_timeout()` | ⚠️ Polling loop with `thread::sleep(10µs)` — not real concurrency |
| `SchedulerMap` | Multi-scheduler keyed by ID for multiple goroutines | ⚠️ Sequential only |
| `GoFuture` | Closure-as-future (implements `std::future::Future`) | |
| `GoGc<T>` | Arc-based reference counting (simulates Go's GC pointers) | |

---

## Runtime Concurrency Note

We love `crossbeam` and `rayon` — we'd love to leverage them properly in the
runtime. The current primitives (`GoScheduler`, `GoChannel`, `GoSelect`) are
**sequential simulations**, not real concurrent primitives:

- `GoScheduler::run()` executes goroutines **sequentially on the current thread**
- `GoSelect` does polling with `thread::sleep(10µs)` spin-waiting
- These are fine for unit tests of algorithmic logic but **do not demonstrate or enable real concurrent Go code**
- Buffered channels in single-threaded contexts will hang if a receiver is not immediately available

---

## Not Yet Implemented

These Go constructs are **blocked** via `compile_error!` at compile time. They are categorized by priority and complexity.

### 🟢 High Priority (Common Go Patterns)

| Category | Go Syntax | Rust Equivalent | Complexity |
|----------|-----------|-----------------|------------|
| **Closures / Anonymous Functions** | `func() { body }` | `|| { body }` or a named `fn` | Medium |
| **Built-in: `make`** | `make(chan T, cap)` → `GoChannel::<T>::with_capacity(cap)`; `make(map[K]V)` → `HashMap::new()`; `make([]T, len)` → `vec![0; len]` | Low — chan unbuffered & buffered working; map type arg needs parsing fix; slice zero-init works
| **Built-in: `new`** | `new(Foo)` | `Foo::default()` | Low |
| **Built-in: `append`** | `append(slice, 1, 2)` | `slice.extend_from_slice(&[1, 2])` | Medium |
| **Built-in: `copy`** | `copy(dst, src)` | `dst.copy_from_slice(&src)` | Low |
| **Built-in: `delete`** | `delete(m, key)` | `m.remove(&key)` | Low |
| **Built-in: `complex`** | `complex(r, i)` | `std::complex::Complex::new(r, i)` | Low |
| **Built-in: `real` / `imag`** | `real(c)`, `imag(c)` | `c.re()`, `c.im()` | Low |
| **Built-in: `panic`** | `panic("msg")` | `panic!("msg")` | Low |
| **Built-in: `len` / `cap`** | `len(s)` | `s.len() as i32` | ✅ Partial (works for slices only, not maps/channels) |
| **Built-in: `clear`** | `clear(slice)` | `slice.clear()` | **❌ Low** |
| **Built-in: `min` / `max`** | `min(a, b)`, `max(a, b)` | `a.min(b)`, `a.max(b)` | **❌ Low** |

> **Note on builtins**: Only `len`, `cap`, `string(bytes)`, type conversion calls
> (`int(x)`, `bool(x)`, etc.), and `make()` for channels have been implemented.
> Most other builtins emit `compile_error!` at compile time. See the Builtin
> Gap table below for the full list.

## syn::Expr Coverage Gap

The transpiler dispatches on `syn::Expr` variants. Of the ~39 `Expr` variants in
`syn` (version 2.x), this transpiler handles **26** (~67%). The remaining **13
variants** fall through to `compile_error!("unsupported Go form")`. This section
details those gaps.

### Missing `syn::Expr` variants (→ `compile_error!`)

| Variant | What it represents | Go analog | Real-world impact |
|---------|-------------------|-----------|-------------------|
| `Expr::Closure` | Anonymous functions `|x| { body }` | `func() { }`, closures | **Huge** — can't pass callbacks, can't use `slices.SortFunc`, `map` with higher-order functions |
| `Expr::Async` | Async blocks `async { }` | Not in Go directly | Medium — needed for async Rust interop |
| `Expr::Await` | `.await` expression | Not in Go directly | Medium — async code |
| `Expr::Match` | Rust `match` expressions | Handled as Go switch statements | Minor |
| `Expr::Try` | `?` operator | Not in Go directly | Medium — error propagation |
| `Expr::TryBlock` | `try { }` blocks | Not in Go directly | Low |
| `Expr::Continue` | Bare `continue` with label | `for { continue }` | Low |
| `Expr::Repeat` | `[x; n]` array repeat | `make([]T, len)` | Medium — needed for `make([]T, len)` |
| `Expr::Reference` | `&expr` references (beyond types) | `&x` pointer ops | Medium — can't dereference pointers in expressions |
| `Expr::RawAddr` | `&raw mut x` | Not in Go | Low |
| `Expr::Unsafe` | `unsafe { }` blocks | Not in Go | Medium — FFI, raw pointer manipulation |
| `Expr::Box` | `box expr` (unstable feature) | Not in Go | Low |
| `Expr::Const` | `const { }` blocks | Not in Go | Low |

### Builtin Gap — common Go builtins with no transpiler support

| Builtin | Status | Frequency in real Go code | Notes |
|---------|--------|--------------------------|-------|
| `append` | ❌ | **Very common** | Idiomatic Go slice mutation |
| `copy` | ❌ | Common | Slice copying |
| `new` | ❌ | Common | Pointer allocation |
| `delete` | ❌ | Common | Map removal |
| `make` (all types) | ⚠️ Partial | Common | Channels work; maps/slices are fragile (raw string parsing for types) |
| `panic` | ❌ | **Common** | Crash semantics |
| `recover` | ❌ | Uncommon | Panic recovery |
| `complex` | ❌ | Low | Complex numbers |
| `real` / `imag` | ❌ | Low | Complex number accessors |
| `clear` | ❌ | Low | Slice/map clearing |
| `min` / `max` | ❌ | Low | Numeric min/max (Go 1.21+) |

> **Note**: The existing `make()` support for channels works via a fragile raw
> string parsing path in `go_stmt_to_rust`. It does not handle complex type
> arguments robustly and may silently miscompile on edge cases.

---

## Standard Library Gap

This is the single biggest barrier to real-world usage. The transpiler operates
at the expression/statement level only. If you reference **any** standard library
function that is not hand-crafted in the builtin dispatcher, you get
`compile_error!("TODO: unsupported Go form")`.

Every real-world Go program imports at least one of: `fmt`, `os`, `io`, `sort`,
`strings`, `sync`, `time`, `math`, `encoding/json`, `net/http`, or similar.
None of these are supported. There is no mechanism for mapping standard library
calls to Rust equivalents — the transpiler has no symbol resolution or AST
classification for library calls.

---

## Real-World Viability

### What kind of Go code WOULD work?

Simple, algorithmic code written in a restricted style:
- Competitive programming solutions
- Educational algorithm demos (sorting, graph traversal, simple search)
- Numeric/computational kernels with no standard library
- Small CLI tools that only use builtins

**Example that works:**
```go
go! {
    func fibonacci(n int) int {
        if n <= 1 {
            return n
        }
        a := 0
        b := 1
        for i := range 2..n {
            next := a + b
            a = b
            b = next
        }
        return b
    }
}
```

**Example that would NOT work:**
```go
go! {
    func main() {
        // ❌ fmt is not supported
        fmt.Println("hello")

        // ❌ append is not supported
        items := append(items, newItem)

        // ❌ defer is not supported
        defer file.Close()

        // ❌ error handling pattern not supported
        data, err := json.Unmarshal(input)
        if err != nil {
            log.Fatal(err)
        }

        // ❌ closures not supported
        slices.SortFunc(items, func(a, b Item) int {
            return a.Name < b.Name
        })
    }
}
```

### Coverage estimate

| Metric | Rating |
|--------|--------|
| **Languages covered** | Go → Rust only |
| **Real-world Go code coverage** | **~5–10%** |
| **Ready for production use** | **No** |
| **Good as a learning exercise** | **Yes** — excellent |
| **Good as a DSL boundary** | **Maybe** — for constrained algorithmic domains |
| **syn::Expr coverage** | **26 of ~39** variants (~67%) |
| **Test file coverage** | **~1,404 lines** of tests, but **~40% is commented-out TODO stubs**; only ~20 tests actually run |

### Code quality assessment

| Aspect | Rating | Notes |
|--------|--------|-------|
| **Parsing** | 4/5 | Careful fork/advance pattern, handles grouped params, switch parsing is competent |
| **Transpiler logic** | 3/5 | Works for the test cases, but edge cases will emit `compile_error!` |
| **Test coverage** | 2/5 | ~40% of the test file is commented-out "TODO" stubs |
| **Documentation** | 4/5 | Roadmap is thorough, AGENTS.md has detailed gotchas |
| **Architecture** | 3/5 | Dual crate (proc-macro + core) is correct, `gourd-check` add-on adds complexity |
| **Runtime primitives** | 2/5 | Channels/select work as simulations but don't enable real concurrent Go code |

### 🟡 Medium Priority (Language Features)

| Category | Go Syntax | Rust Equivalent | Complexity |
|----------|-----------|-----------------|------------|
| **Defer** | `defer cleanup()` | Custom closure on drop guard | **High** — critical Go pattern |
| **Variadic parameters** | `func foo(args ...int)` | `&[i32]` or explicit impl | Medium |
| **Type switch** | `switch x.(type) { case int: ... }` | `match x { Type::Int => ... }` | Medium |
| **Goto** | `goto label; label: stmt` | No Rust equivalent | Low | ⚠️ Rarely used, can be refactored |
| **Labels on loops** | `loop: for { break loop }` | No Rust equivalent | Low | ⚠️ Rarely used, can be refactored |
| **Pointer types** | `*int` | `&i32` | Medium | ⚠️ Pointer types (types only) are handled; pointer *expression* operations are not.
| **`sync.Mutex`** | `sync.Mutex{}` | `std::sync::Mutex` | Medium |
| **`sync.WaitGroup`** | `sync.WaitGroup{}` | `std::sync::Barrier` + atomic | High |
| **`sync.RWMutex`** | `sync.RWMutex{}` | `std::sync::RwLock` | Medium |
| **Interface embedding** | `type Reader struct { Writer }` | Trait inheritance (not supported in Rust) | High |
| **Type aliases** | `type MyInt int` | Rust `type` aliases | Low |
| **Constant declarations** | `const PI = 3.14159` | Rust `const` | Low |
| **Variable declarations** | `var x int = 42` | Rust `let` / `mut` | Low |

### 🔴 Low Priority / Advanced

| Category | Go Syntax | Rust Equivalent | Complexity |
|----------|-----------|-----------------|------------|
| **Generics** | `func Min[T ordered](a, b T) T` | Rust type parameters | High | ⚠️ Hard but needed |
| **`defer recover()`** | `defer func() { if r := recover(); r != nil { ... } }()` | `std::panic::catch_unwind()` | High | Medium |
| **`range` over channels** | `for v := range ch { ... }` | Channel iteration | Medium | Low |
| **Async/await** | `go func() { ... }`, `await` | No direct equivalent | High | Medium |
| **Error handling** | `if err != nil { ... }` | `?` operator | Medium | High |
| **Package/Import** | `package main`, `import "fmt"` | No equivalent in inline mode | Low | Low |
| **Interface empty** | `interface{}` | `()` or `Box<dyn Any>` | Medium | Medium |
| **Untyped constants** | `const Pi = 3.14159` | Type resolution needed | Medium | Low |
| **Use declarations** | `use "fmt"` in Go | No equivalent | Low | Low |
| **Import aliases** | `import f "fmt"` | No equivalent | Low | Low |
| **`panic` / `recover`** | `panic("msg")`, `recover()` | `panic!()` | Low | High |
| **`complex` / `real` / `imag`** | `complex(r, i)`, `real(c)`, `imag(c)` | `std::num::Complex` | Low | Low |

---

## Architecture

```
gourd/
  gourd-codegen/       <-- proc-macro library (transpiler core)
  gourd/               <-- runtime + CLI tool (`gourd transpile`)
  gourd-check/         <-- standalone Go/Rust validation tool
  gourd-codegen-core/  <-- core transpiler logic
```

### Data flow

1. User writes: `go! { fn hello() string { String::from("hello") } }`
2. The `go!` proc-macro inspects tokens to dispatch to the transpiler
3. Transpiler converts Go types, params, bounds, and bodies to Rust
4. Emits pure `quote! { fn hello() -> String { String::from("hello") } }`
5. Unsupported forms expand to `compile_error!` with clear error messages
