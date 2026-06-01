# Gourd

Transpiles inline Go declarations into valid Rust via a procedural macro at compile time.

# ROADMAP

This document outlines the current state and remaining features for Go → Rust compatibility.

## Implemented Features

The following Go constructs are fully transpiled and tested:

| Category | Go Syntax | Rust Output |
|----------|-----------|-------------|
| **Literals** | `42`, `0xff`, `1e3`, `"hello"`, `true`/`false` | Direct Rust equivalents |
| **Binary operators** | `+ - * / % ^ & | << >> == != > >= < <=` | Direct Rust equivalents |
| **Unary operators** | `-` (negation), `!` (not), `*` (dereference) | Direct Rust equivalents |
| **Control flow** | `if cond { ... } else { ... }` | Direct Rust `if/else` |
| **Switch** | `switch n { case 1: "one" default: "other" }` | `match n { 1 => "one", _ => "other" }` |
| **Loops** | `for i, v := range data` | `for (i, v) in data.iter().copied().enumerate()` |
| **While loops** | `while cond { ... }` | `while cond { ... }` |
| **Continue** | `continue` | `continue` |
| **Functions** | `func hello(a int, b int) int { return a + b }` | `fn hello(a: i32, b: i32) -> i32 { return a + b }` |
| **Param grouping** | `func foo(a, b, c int)` | `fn foo(a: i32, b: i32, c: i32)` |
| **Multi-return** | `func divmod(n, d int) (int, int)` | `fn divmod(n: i32, d: i32) -> (i32, i32)` |
| **Receiver methods** | `func (f Foo) Bar(z int) int { f.x }` | `impl Foo { fn Bar(&self, z: i32) -> i32 { self.x } }` |
| **Pointer receivers** | `func (f *Foo) Baz(z int) int { f.x = f.x + z }` | `impl Foo { fn Baz(&mut self, z: i32) -> i32 { self.x = self.x + z } }` |
| **Structs** | `struct Foo { x int }` | `struct Foo { pub x: i32 }` |
| **Field assignment** | `c.count = c.count + 1` | `self.count = self.count + 1` |
| **Slice literals** | `[]int{1, 2, 3}` | `vec![1, 2, 3]` |
| **Map literals** | `map[string]int{"a": 1}` | `HashMap::new(); m.insert(...)` |
| **Map access** | `m[key]`, `m.get(key)` | `m[&key]`, `m.get(&key)` |
| **Short declarations** | `x := y` | `let x = y` |
| **Assignments** | `x = y` | `x = y` |
| **Type conversions** | `int()`, `uint()`, `float32()`, `float64()`, `bool()`, `byte()`, `rune()`, `string()` | Rust cast/convert equivalents |
| **Tuple/multi values** | `return a, b` | `return (a, b)` |
| **Indexing** | `v[i]`, `m[k]`, `m[k1][k2]` | `v[i]`, `m[&k]` |
| **Blocks** | `{ stmt; expr }` | `{ stmt; expr }` |
| **Method calls** | `s.method(args)` | `s.method(args)` |
| **Field access** | `pt.0`, `s.field` | `pt.0`, `s.field` |
| **Interfaces** | `interface Foo { Name() string }` | `trait foo { fn name(&self) -> String; }` |

### Type Mappings

| Go | Rust |
|----|------|
| `int`, `int8`, `int16` | `i8`, `i16` |
| `int32` | `i32` |
| `int64` | `i64` |
| `uint`, `uint8`, `uint16` | `u8`, `u16` |
| `uint32` | `u32` |
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

---

## Implemented Tools

| Tool | Purpose |
|------|---------|
| `go!` proc-macro | Transpiles inline Go → Rust at compile time |
| `#[verify_rust_output({ expected })]` | Compile-time assertion: transpiled output must match expected Rust |
| `gourd transpile` | CLI tool: transpile Go blocks from source files or stdin |
| `gourd-check` | Standalone validator: checks Go syntax via `go build`, Rust via `cargo check` |
| `cargo expand` | See expanded `go!` → Rust output in your crate |

---

## Not Yet Implemented

These Go constructs are **blocked** via `compile_error!` at compile time:

| Category | What's missing | Notes |
|----------|----------------|-------|
| **Concurrency** | `go func()`, `chan`, `select` | Go's concurrency model ≠ Rust's |
| **Goroutines** | `go foo(42)` | Would require async/spawn |
| **Channels** | `ch <- value`, `<- ch` | Rust uses different channel patterns |
| **`defer`** | `defer cleanup()` | No Rust equivalent |
| **`panic`/`recover`** | `panic("msg")`, `recover()` | Rust has `panic!` / `catch_unwind` |
| **`sync` primitives** | `sync.Mutex`, `sync.WaitGroup` | No sync crate mapping |
| **Type assertions** | `x.(int)` | Rust `as` casts work; type switches not yet |
| **Labels** | `loop: for { break loop }` | Go labels → Rust has no labels |
| **Function declarations inside blocks** | Nested function definitions | Rust closures only |
| **Empty bare structs** | `struct{}` | Undersized, empty structs |
| **`assert` statements** | `assert.Equal(a, b)` | Testing library, not language feature |
| **`string(bytes)` conversions** | `string([]byte{...})` | UTF-8 validation not yet |
| **Map/Slice in signatures** | `func foo(data []int)` | Partial (slice type detection works, but arg handling incomplete) |

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
