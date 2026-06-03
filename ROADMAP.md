# 🍂 Gourd — Roadmap

> Write Go. Get Rust. At compile time.

This document outlines the current state and remaining features for Go → Rust compatibility.

## Implemented Features

The following Go constructs are fully transpiled and tested:

### Function Declarations

| Go | Rust | Notes |
|----|------|-------|
| `func foo(a int, b int) int { ... }` | `fn foo(a: i32, b: i32) -> i32 { ... }` | |
| `func foo(a, b, c int) int { ... }` | `fn foo(a: i32, b: i32, c: i32) -> i32 { ... }` | Parameter grouping |
| `func divmod(n, d int) (int, int) { ... }` | `fn divmod(n: i32, d: i32) -> (i32, i32) { ... }` | Multi-return |
| `func (f Foo) Method(z int) int { ... }` | `impl Foo { fn Method(&self, z: i32) -> i32 { ... } }` | Value receiver |
| `func (f *Foo) Method(z int) int { ... }` | `impl Foo { fn Method(&mut self, z: i32) -> i32 { ... } }` | Pointer receiver |

### Struct & Interface Definitions

| Go | Rust | Notes |
|----|------|-------|
| `struct Foo { x int, y int }` | `struct Foo { pub x: i32, pub y: i32 }` | Fields auto-`pub` |
| `interface Shape { Name() string }` | `trait shape { fn name(&self) -> String; }` | |
| `interface Reader { Read(data []byte) []byte }` | `trait reader { fn read(&self, data: &[u8]) -> Vec<u8>; }` | Parameter grouping + slice types |

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
| `len(s)` | `s.len() as i32` | |
| `cap(s)` | `s.len() as i32` | |
| `string(bytes)` | `std::str::from_utf8(&bytes).unwrap_or("").to_string()` | `[]byte` → `String` |
| `x.(T)` | `x as T` (with type-specific coercion) | Type assertion |
| `return a, b` | `return (a, b)` | Multi-return |
| `x := y` | `let x = y` | Short declaration |
| `x = y` | `x = y` | Assignment |

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
| `map[string]int{"a": 1}` | `HashMap::new(); m.insert("a", 1)` | |
| `m[key]`, `m.get(key)` | `m[&key]`, `m.get(&key)` | Reference for map access |

### Concurrency (crossbeam-backed)

| Go | Rust | Notes |
|----|------|-------|
| `go func() { ... }` | `GoScheduler::new().submit(|| { ... })` | Sequential goroutine simulation |
| `chan int` | `GoChannel::<i32>::new()` | Unbuffered channel |
| `chan int{10}` | `GoChannel::<i32>::with_capacity(10)` | Buffered channel |
| `ch <- value` | `ch.send(value)` | Send to channel |
| `return <-ch` | `return ch.recv().unwrap()` | Receive from channel |
| `select { case ... }` | `GoSelect::<T>::new().run()` | Channel select with default/timeout |

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
| `GoScheduler` | Thread-safe task scheduler — `submit()`, `run()`, `clone()` |
| `GoChannel<T>` | Generic channel — `send()`, `recv()`, `try_send()`, `try_recv()`, `with_capacity()` |
| `GoSelect<T>` | Select with `send_case()`, `recv_case()`, `with_default()`, `with_timeout()` |
| `SchedulerMap` | Multi-scheduler keyed by ID for multiple goroutines |
| `GoFuture` | Closure-as-future (implements `std::future::Future`) |
| `GoGc<T>` | Arc-based reference counting (simulates Go's GC pointers) |

---

## Not Yet Implemented

These Go constructs are **blocked** via `compile_error!` at compile time. They are categorized by priority and complexity.

### 🟢 High Priority (Common Go Patterns)

| Category | Go Syntax | Rust Equivalent | Complexity |
|----------|-----------|-----------------|------------|
| **Closures / Anonymous Functions** | `func() { body }` | `|| { body }` or a named `fn` | Medium |
| **Struct Literals** | `Point{x: 1, y: 2}` | `Point { x: 1, y: 2 }` | Medium |
| **Built-in: `make`** | `make(chan int, 10)` | `GoChannel::<i32>::with_capacity(10)` | Low |
| **Built-in: `new`** | `new(Foo)` | `Foo::default()` | Low |
| **Built-in: `append`** | `append(slice, 1, 2)` | `slice.extend_from_slice(&[1, 2])` | Medium |
| **Built-in: `copy`** | `copy(dst, src)` | `dst.copy_from_slice(&src)` | Low |
| **Built-in: `delete`** | `delete(m, key)` | `m.remove(&key)` | Low |
| **Built-in: `complex`** | `complex(r, i)` | `std::complex::Complex::new(r, i)` | Low |
| **Built-in: `real` / `imag`** | `real(c)`, `imag(c)` | `c.re()`, `c.im()` | Low |
| **Built-in: `panic`** | `panic("msg")` | `panic!("msg")` | Low |
| **Built-in: `len` / `cap`** | `len(s)` | `s.len() as i32` | ✅ Partial (works for slices) |
| **Built-in: `clear`** | `clear(slice)` | `slice.clear()` | Low |
| **Built-in: `min` / `max`** | `min(a, b)`, `max(a, b)` | `a.min(b)`, `a.max(b)` | Low |

### 🟡 Medium Priority (Language Features)

| Category | Go Syntax | Rust Equivalent | Complexity |
|----------|-----------|-----------------|------------|
| **Defer** | `defer cleanup()` | Custom closure on drop guard | High |
| **Variadic parameters** | `func foo(args ...int)` | `&[i32]` or explicit impl | Medium |
| **Type switch** | `switch x.(type) { case int: ... }` | `match x { Type::Int => ... }` | Medium |
| **Goto** | `goto label; label: stmt` | No Rust equivalent | Low |
| **Labels on loops** | `loop: for { break loop }` | No Rust equivalent | Low |
| **Pointer types** | `*int` | `&i32` | Medium |
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
| **Generics** | `func Min[T ordered](a, b T) T` | Rust type parameters | High |
| **`defer recover()`** | `defer func() { if r := recover(); r != nil { ... } }()` | `std::panic::catch_unwind()` | High |
| **`range` over channels** | `for v := range ch { ... }` | Channel iteration | Medium |
| **Async/await** | `go func() { ... }`, `await` | No direct equivalent | High |
| **Error handling** | `if err != nil { ... }` | `?` operator | Medium |
| **Package/Import** | `package main`, `import "fmt"` | No equivalent in inline mode | Low |
| **Interface empty** | `interface{}` | `()` or `Box<dyn Any>` | Medium |
| **Untyped constants** | `const Pi = 3.14159` | Type resolution needed | Medium |
| **Use declarations** | `use "fmt"` in Go | No equivalent | Low |
| **Import aliases** | `import f "fmt"` | No equivalent | Low |

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
