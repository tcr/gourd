# Gourd

Transpiles inline Go declarations into valid Rust at compile time.

## Quick start

Write Go-style code in a `go!` block and it becomes valid Rust:

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

### Control flow

- **If / else**: `if cond { body } else { body }`
- **Switch**: selector-based (`switch n { case 1, 2: "one_or_two" }`) and boolean (`switch { case ok: "ok" }`)
- **Loops**: `while`, `for` (range and index variants), `continue`, `break`

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

## Unsupported forms

Missing Go constructs expand to a compile-time error:

```
error: TODO: transpile this Go form: channels
```

Not yet implemented: channels, goroutines, interfaces, `defer`, `panic`, labels, `string(byte_slice)` conversions, closures, async/await.

## Running tests

```bash
cargo test   # → 50 tests (go! transpilation verify + functional runtime tests + gourd-check)
gourd transpile "func hello() int { return 42 }"  # → transpile CLI tool
cargo expand -p gourd  # → see expanded Go → Rust transpilation.
```
