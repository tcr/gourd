# Gourd

Transpiles inline Go declarations into valid Rust via a procedural macro at compile time.

## Architecture

```
gourd/
  gourd-codegen/       <-- proc-macro library (transpiler core)
  gourd/               <-- demo binary using `go! { ... }`
```

[`gourd-codegen/src/transpiler.rs`]  -- Go → Rust transpiler
[`gourd-codegen/src/lib.rs`]         -- `#[proc_macro]` entry (`go!`)

## How it works

1. User writes: `go! { fn hello() string { String::from("hello") } }`
2. The proc-macro `go!` inspects the tokens to dispatch to the correct handler
3. The transpiler converts Go type names, parameters, bounds, and bodies to Rust
4. Emits pure `quote! { fn hello() -> String { String::from("hello") } }` — no runtime dependency.

Every Go declaration in the source can be valid Rust tokens. The macro
emits exactly those tokens into the user's expanded AST.

### Supported forms

| Go form | Rust transpilation |
|---------|-------------------|
| `fn foo(a, b int) int { ... }` | `fn foo(a: i32, b: i32) -> i32 { ... }` |
| `struct Point { x int, y int }` | `struct Point { pub x: i32, pub y: i32 }` |
| `func (f Foo) Method() int { ... }` | `impl Foo { fn Method(&self) -> i32 { ... } }` |
| `nil` | `None` |
| `x := y` (short decl) | `let x = y` |
| ... (more below) ... |

### Unsupported forms → `compile_error!`

Any Go concept missing from the transpiler (e.g. concurrency, storage, streams, etc.) expands to a `compile_error!` that
reports "TODO: transpile this Go form: <name>" at compile time of the
consumer's crate.

## Running

```bash
cargo test      # → go! integration tests
cargo run -p gourd  # → demo binary
cargo expand -p gourd  # → see expanded Go → Rust transpilation.
```

### Supported language features

The `go!` macro supports:

- **Function declarations**: `fn name(params) output { body }` — Go type names mapped to Rust equivalents
- **Struct declarations**: `struct Name { field type, ... }` — fields made `pub`
- **Receiver functions**: `func (recv Type) name(params) { body }` — converted to `impl Type` blocks
- **Go-style parameter grouping**: `func foo(a, b, c int) { ... }` — multiple params share one type
- **Slice type shorthand**: `a []int` — maps to `a: &[i32]`
- **Multi-return values**: `func foo() (int, string) { ... }` — maps to `-> (i32, String)`
- **Go to Rust type mapping**:  
  `int→i32, int8→i8, string→String, bool→bool, error→Box<dyn std::error::Error>`, etc.

### Expression support inside `go!` function bodies

When transpiling the body of a `go!` function, the following Go constructs are supported:

| Go form | Rust transpilation |
|---------|-------------------|
| `len(s)` | `s.len() as i32` |
| `nil` | `None` |
| `x := y` (short decl) | `let x = y` |
| `if cond { ... } else { ... }` | `if cond { ... } else { ... }` |
| `if cond { expr1 } else { expr2 }` (expression/if-else) | `if cond { expr1 } else { expr2 }` |
| `break label`, `return expr` | same |
| Binary operators: `+ - * / % && || ^ & | << >> == != < <= > >=` | same |
| Unary operators: `- ! *` (neg, not, deref) | same |
| Array/index: `s[i]` | same |
| Method calls: `s.len()` | same |
| Tuple: `(a, b)` | same |
| Cast: `x as T` | same |
| Assignment: `x = y` | same |
gourd/
  gourd-codegen/       <-- proc-macro library (transpiler core)
  gourd/               <-- demo binary using `go_expr! { ... }`
```

[`gourd-codegen/src/transpiler.rs`]  -- Go → Rust transpiler
[`gourd-codegen/src/lib.rs`]         -- `#[proc_macro]` entry (`go_expr!`)

## How it works

1. User writes: `go_expr! { 10 + 20 }`
2. The proc-macro `go_expr! { ... }` binds from tokens per `syn::Expr`
3. The transpiler dispatches on the AST node: `Expr::Binary → BinOp::Add → #lhs + #rhs`
4. `syn::Expr` (Go e.g. `10 + 20i32` (literal fork) → Rust valid output
5. Emits pure `quote! { 10 + 20 }` — no runtime dependency.

Every expression in the Go source can be valid Rust tokens. The macro
emits exactly those tokens into the user's expanded AST.

### Supported forms

| Go form | Rust transpilation |
|---------|-------------------|
| `10 + 20` | `10 + 20` |
| `len(s)` | `s.len()` |
| `nil` | `None` |
| `x := y` (short decl) | `let x = y` |
| ... (more below) ... |

### Unsupported forms → `compile_error!`

Any Go concept missing from the transpiler (e.g. struct declarations,
concurrency, storage, streams, etc.) expands to a `compile_error!` that
reports "TODO: transpile this Go form: <name>" at compile time of the
consumer's crate.

## Running

```bash
cargo test     # → arithmetic integration tests
cargo test -p gourd  # → demo binary
cargo expand -p gourd  # → see expanded Go → Rust transpilation.
