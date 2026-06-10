# 🍂 Gourd

Go to Userland Rust — at compile time via a procedural macro.

> ⚠️ **EXPERIMENTAL — NOT PRODUCTION READY**
> Gourd is an early experimental project. It transpiles **basic Go syntax** into Rust. Real-world Go code coverage is ~5–8%. It is **not suitable for most Go projects**. If you need a reliable Go-to-Rust tool, look elsewhere. If you want a fun toy to tinker with, read on.

## Why Gourd?

Write Go-style code in `go! { ... }` blocks. At compile time, a procedural macro transpiles them into valid Rust. No external build steps, no code generation tools.

- **Familiar syntax** — write Go declarations, get Rust implementations.
- **Type-safe** — Go types map directly to Rust equivalents (`int` → `i32`, `string` → `String`).
- **Macro-powered** — no external build steps, no code generation tools.
- **Standalone validation** — `gourd-check` validates Go syntax before compilation.
- **Real concurrency** — crossbeam-backed channels, scheduler, and select primitives.

## Quick Start

```rust
use gourd::go;

go! {
    func goAdd(a int, b int) int {
        return a + b
    }
}

fn main() {
    assert_eq!(goAdd(2, 3), 5);
}
```

That's it. The Go code is transpiled to Rust at compile time — no runtime overhead.

Go names are preserved as camelCase in the output (`goAdd` → `goAdd`, not `go_add`).

## CLI: `gourd transpile`

Transpile Go code to Rust from inline input, stdin, or files:

```bash
gourd transpile "func hello() int { return 42 }"
echo "func hello() int { return 42 }" | gourd transpile -
gourd transpile path/to/file.rs
```

## Module layout

```
gourd/
  gourd-macro/       <-- proc-macro library (transpiler core)
  gourd/               <-- runtime + CLI tool (`gourd transpile`)
  gourd-codegen/       <-- shared transpiler library (scanner + transpiler)
  gourd-check/         <-- standalone Go/Rust validation CLI
```

### gourd runtime (`gourd/src/`)

The runtime is organized into three layers:

| Layer | Module | Contents |
|-------|--------|----------|
| Root types | `go_gc.rs`, `go_scheduler.rs` | `GoGc`, `GoScheduler`, `GoChannel`, `GoSelect`, `SchedulerMap`, `GoFuture` — exported at crate root for generated code |
| Prelude | `prelude/` | Runtime types for user code: `GoDeferGuard`, `GoMutex`, `GoRc`, `GoOnce`, `GoWaitGroup`, `GoRWMutex`, `GoError`, `Any`, `GoRand`, formatting helpers (`fmt_sprintf`, `fmt_print`, `fmt_println`, `fmt_printf`), builtins (`len`, `cap`, `append`, `make_slice`, `make_map`, `copy`, `min`, `max`, `std_copy`, `std_delete`, `std_append`) |
| Packages | `packages/` | Go stdlib package emulation: os, strings, json, io, bytes, math, byte, time |

Import paths:
- Root primitives: `gourd::GoGc`, `gourd::GoScheduler`, `gourd::GoChannel<T>`, `gourd::GoSelect<T>`, `gourd::SchedulerMap`, `gourd::GoFuture`
- Prelude types: `gourd::prelude::*` (GoMutex, GoRc, GoError, Any, etc.)
- Package emulation: `gourd::packages::*` (os_open, strings_replace, etc.)

### CLI (`gourd transpile`)

The CLI tool (`gourd/src/main.rs`) supports:
- Inline Go code: `gourd transpile "func hello() int { return 42 }"`
- File paths: `gourd transpile path/to/file.rs`
- Stdin: `echo "..." \| gourd transpile -`

## Supported constructs

### Closures

| Go | Rust |
|----|------|
| `f := func() { body }` | `let f = || { body };` |
| `f := func(x int) int { body }` | `let f = |x: i32| -> i32 { body };` |
| `f := func(arr []int) int { body }` | `let f = |arr: &[i32]| -> i32 { body };` |
| `f := func() (a, b int) { body }` | `let f = || -> (i32, i32) { body };` |

*Note: Go builtins (`len`, `[]` indexing) inside closure bodies are not yet transpiled.*

### Variadic parameters

| Go | Rust |
|----|------|
| `func foo(nums ...int) int { ... }` | `fn goFoo(nums: &[i32]) -> i32 { ... }` |
| `func foo(min int, nums ...int) int { ... }` | `fn goFoo(min: i32, nums: &[i32]) -> i32 { ... }` |

Variadic `...T` parameters are mapped to slice references `&[T]`.

### Function declarations

| Go | Rust |
|----|------|
| `func goAdd(a int, b int) int { ... }` | `fn goAdd(a: i32, b: i32) -> i32 { ... }` |
| `func goSum(a, b, c int) int { ... }` | `fn goSum(a: i32, b: i32, c: i32) -> i32 { ... }` |
| `func (f Foo) Method(z int) int { ... }` | `impl Foo { fn Method(&self, z: i32) -> i32 { ... } }` |
| `func (f *Foo) Method(z int) int { ... }` | `impl Foo { fn Method(&mut self, z: i32) -> i32 { ... } }` |
| `return a, b` | `return (a, b)` (multi-return) |

### Structs and interfaces

| Go | Rust |
|----|------|
| `struct Point { x int, y int }` | `struct Point { pub x: i32, pub y: i32 }` |
| `interface Shape { Name() string }` | `trait Shape { fn name(&self) -> String; }` |

### Types

| Go | Rust |
|----|------|
| `int`, `int8`–`int64` | `i8`–`i64` |
| `uint`, `uint8`–`uint64` | `u8`–`u64` |
| `uintptr` | `usize` |
| `string`, `bool`, `byte`, `rune` | `String`, `bool`, `u8`, `char` |
| `float32`, `float64` | `f32`, `f64` |
| `[]T` (slice type) | `&[T]` |
| `chan T` | `GoChannel::<T>::new()` |
| `error` | `Box<dyn std::error::Error>` |

### Expressions and builtins

| Go | Rust |
|----|------|
| `len(s)`, `cap(s)` | `s.len() as i32`, `s.capacity() as i32` |
| `string(bytes)` | `String::from_utf8(bytes)` |
| `int(x)`, `bool(x)`, etc. | explicit casts |
| `make(chan/map/slice)` | `GoChannel::new()`, `HashMap::new()`, `Vec::new()` |
| `new(Foo)` | `Foo::default()` |
| `panic("msg")` | `panic!("msg")` |
| `append(slice, items)` | push to a Vec copy |
| `[]int{1, 2, 3}` | `vec![1, 2, 3]` |
| `map[string]int{...}` | `HashMap` + inserts |
| `x.(T)` (type assertion) | type cast/downcast |
| `&T`, `*p` (pointers) | `&T`, `*p` via `UnOp` |

### Control flow

- `if / else / else if`
- `switch / match` (selector and no-selector forms)
- `for` with `range` (index-only and index+value forms)
- `while`
- `continue`, `break`
- `return` (single and multi-return)
- Struct literals, map literals, slice literals, ranges
- `match` expressions (Rust native)

### Concurrency (crossbeam-backed)

| Go | Rust |
|----|------|
| `go func() { ... }` | `GoScheduler::new().submit(|| { ... })` |
| `chan int` / `chan int{10}` | `GoChannel::new()` / `GoChannel::with_capacity(10)` |
| `ch <- 42` / `<-ch` | `ch.send(42)` / `ch.recv()` |
| `select { case ... }` | `GoSelect::new().send_case(...).run()` |

Concurrency primitives are real `crossbeam`-backed types — not stubs. The scheduler runs goroutines sequentially (simulating Go's scheduler), channels support `send`, `recv`, `try_send`, `try_recv`, and `select` supports send cases, receive cases, default cases, and timeouts.

### Standard library mappings

The transpiler recognizes and maps common Go standard library packages to Rust equivalents:

| Go Package | Functions Mapped |
|------------|-----------------|
| `strings` | `Replace`, `ReplaceAll`, `HasPrefix`, `HasSuffix`, `Contains`, `Split`, `Join`, `Index`, `LastIndex`, `Trim`, `TrimLeft`, `TrimRight`, `ToUpper`, `ToLower`, `Repeat`, `Fields` |
| `os` | `Open`, `ReadFile`, `WriteFile`, `Mkdir`, `MkdirAll`, `Remove`, `Chdir`, `Getenv`, `Setenv`, `EnvKeys`, `Args` |
| `io` | `Copy`, `ReadAll` |
| `bytes` | `Contains`, `HasPrefix`, `HasSuffix`, `Index`, `Split`, `Join`, `Replace` |
| `encoding/json` (as `json`) | `Marshal`, `Unmarshal` |
| `time` | `Now`, `Since`, `Until`, `Sleep` |
| `math` | `Abs`, `Sqrt`, `Floor`, `Ceil`, `Round`, `Min`, `Max`, `PI`, `E`, `Exp`, `Log`, `Log10`, `Pow`, `Sign` |
| `byte` | `Of`, `RuneOf`, `StringToBytes`, `BytesToString` |

## Compile-time verification

```rust
use gourd::go;

#[verify_rust_output({
    fn goAdd(n: i32) -> i32 {
        return n + 1;
    }
})]
go! {
    func goAdd(n int) int {
        return n + 1
    }
}
```

If the transpiled output doesn't match, compilation fails with expected vs actual.

## Standalone validation

```bash
gourd-check [PATHS...]      # Scan files
gourd-check -g PATHS         # Go-only
gourd-check -r PATHS         # Rust-only
```

## Running

```bash
cargo test                    # ~150 tests in gourd-macro
cargo expand -p gourd         # See expanded transpilation
gourd transpile "func hello() int { return 42 }"
```

> Validation is **enabled by default** via the `validate` feature — every `go!` block is checked with `go build` at compile time. Use `--no-default-features` to disable validation for fast iterations, or use the `gourd-check` CLI for pre-compilation validation.

## Debug Output

Set the `GOURD_DEBUG` environment variable to print diagnostic messages to stderr:

```bash
GOURD_DEBUG=1 gourd transpile "func hello() int { return 42 }"
```

Debug output includes parsing details, type mappings, and transpilation steps. Without `GOURD_DEBUG`, output is clean — only the transpiled Rust tokens.

> **Tip**: Useful for understanding what the transpiler sees when investigating failed transpilation or unexpected output. The flag is runtime-configured (checked at runtime via `std::env::var`), not compile-time — it has zero overhead when unset.

### Status

| Metric | Value |
|--------|-------|
| **Real-world Go coverage** | ~5–8% |

### What's next?

See [ROADMAP.md](ROADMAP.md). The remaining gaps are `defer` dedicated tests, full error handling completeness, `net/http`, `database/sql`, and generics support.
