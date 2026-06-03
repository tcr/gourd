# 🎃 Gourd

GO to Userland Rust Demo

## Why Gourd?

Write Go. Get Rust. At compile time.

> ⚠️ **EXPERIMENTAL — NOT PRODUCTION READY**  
> Gourd is an early, experimental project. It is **not suitable for most Go projects**. The transpiler is incomplete — many Go constructs will fail at compile time with `compile_error!`. There is no API stability guarantee, breaking changes can happen at any time, and the runtime concurrency primitives (scheduler, channels, select) are best-effort simulations of Go's runtime, not drop-in replacements. Use at your own risk. If you need a reliable Go-to-Rust tool, look elsewhere. If you want a fun toy to tinker with, read on.

Gourd lets you write Go-style code and get valid Rust output.

Gourd: a hard outer shell (Golang) that, once processed, becomes something useful and expressive (Rust).

- **Familiar syntax** — write Go declarations, get Rust implementations.
- **Type-safe** — Go types map directly to Rust equivalents (`int` → `i32`, `string` → `String`).
- **Macro-powered** — no external build steps, no code generation tools.
- **Standalone validation** — `gourd-check` validates Go syntax before compilation.

## Quick Start

```rust
use gourd::go;

go! {
    func goAdd(a int, b int) int {
        return a + b
    }
}

fn main() {
    assert_eq!(go_add(2, 3), 5);
}
```

That's it. The Go code is transpiled to Rust at compile time — no runtime overhead.

## CLI: `gourd transpile`

Transpile Go code to Rust from inline input, stdin, or files:

```bash
# Inline Go code
gourd transpile "func goAdd(a int, b int) int { a + b }"

# From stdin
echo "func hello() int { return 42 }" | gourd transpile -

# Rust file with go! blocks
gourd transpile path/to/file.rs
```

## Supported forms

### Declarations

| Go | Rust |
|----|------|
| `fn foo(a, b int) int { ... }` | `fn foo(a: i32, b: i32) -> i32 { ... }` |
| `struct Point { x int, y int }` | `struct Point { pub x: i32, pub y: i32 }` |
| `func (f Foo) Method() int { ... }` | `impl Foo { fn Method(&self) -> i32 { ... } }` |
| `interface Foo { Name() string }` | `trait foo { fn name(&self) -> String; }` |
| `chan int` | `GoChannel::<i32>::new()` |
| `chan []int` | `GoChannel::<Vec<i32>>::new()` |
| `select { case ... }` | `GoSelect::new().run()` |

### Types

| Go | Rust |
|----|------|
| `int`, `int8`, `int16`, `int32`, `int64` | `i32`, `i8`, `i16`, `i32`, `i64` |
| `uint`, `uint8`, `uint16`, `uint32`, `uint64` | `u32`, `u8`, `u16`, `u32`, `u64` |
| `string` | `String` |
| `bool` | `bool` |
| `byte` | `u8` |
| `rune` | `char` |
| `float32`, `float64` | `f32`, `f64` |
| `error` | `Box<dyn std::error::Error>` |
| `[]T` | `&[T]` |

### Expressions

| Go | Rust |
|----|------|
| `nil` | `None` |
| `x := y` | `let x = y` |
| `len(s)` | `s.len() as i32` |
| `if cond { ... } else { ... }` | `if cond { ... } else { ... }` |
| `switch x { case 1: ... }` | `match x { 1 => ... }` |
| `while cond { ... }` | `while cond { ... }` |
| `for i, v := range data { ... }` | `for (i, v) in data.iter().copied().enumerate() { ... }` |
| `for i := range data { ... }` | `for i in 0..data.len() { ... }` |
| `continue` | `continue` |
| `break` | `break` |
| `return expr` | `return expr` |
| `[]int{1, 2, 3}` | `vec![1, 2, 3]` |
| `map[string]int{"a": 1}` | `HashMap::new(); m.insert("a", 1)` |
| `x as T` (cast) | `x as T` |
| `x[i]` (index) | `x[i as usize]` |
| `x + y`, `x - y`, etc. | same |
| `go func() { ... }` | `GoScheduler::new().submit(|| { ... })` |

### Control flow

- **If / else**: `if cond { body } else { body }`
- **Switch**: selector-based (`switch n { case 1, 2: "one_or_two" }`) and boolean (`switch { case ok: "ok" }`)
- **Loops**: `while`, `for` (range and index variants), `continue`, `break`

### Concurrency (crossbeam-powered)

| Go | Rust | Notes |
|----|------|-------|
| `go func() { ... }` | `GoScheduler::new().submit(|| { ... })` | Sequential goroutine simulation |
| `chan int` | `GoChannel::<i32>::new()` | Unbuffered channel |
| `chan int{10}` | `GoChannel::<i32>::with_capacity(10)` | Buffered channel |
| `ch <- 42` | `ch.send(42)` | Send to channel |
| `<-ch` | `ch.recv()` | Receive from channel |
| `select { case ... }` | `GoSelect::<T>::new().run()` | Channel select |

Concurrency primitives are real `crossbeam`-backed types — not stubs. The scheduler runs goroutines sequentially (simulating Go's scheduler), channels support `send`, `recv`, `try_send`, `try_recv`, and `select` supports send cases, receive cases, default cases, and timeouts.

## Compile-time verification

Use `#[verify_rust_output({ ... })]` to assert at compile time that your Go code transpiles correctly:

```rust
use gourd::go;

#[verify_rust_output({
    fn go_add(n: i32) -> i32 {
        return n + 1;
    }
})]
go! {
    func goAdd(n int) int {
        return n + 1
    }
}
```

If the transpiled output doesn't match, compilation fails with a clear error showing expected vs actual.

## Standalone validation with `gourd-check`

Validate Go syntax inside `go!` blocks without running the full test suite:

```bash
gourd-check [PATHS...]      # Scan files (default: current directory)
gourd-check -g PATHS         # Go-only validation
gourd-check -r PATHS         # Rust-only validation
```

## Running tests

```bash
cargo test           # run all tests
cargo test --lib     # unit tests only
cargo expand -p gourd # see expanded Go → Rust transpilation.
gourd transpile "func hello() int { return 42 }"  # transpile CLI tool
```
