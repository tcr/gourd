# Gourd

Transpiles inline Go expressions into valid Rust via a procedural macro at compile time.

## Architecture

```
gourd/
  gourd-codegen/       <-- proc-macro library (transpiler core)
  gourd/               <-- demo binary using `go! { ... }`
  gourd-embed-tests/   <-- integration tests
```

[`gourd-codegen/src/transpiler.rs`]  -- Go → Rust transpiler
[`gourd-codegen/src/lib.rs`]         -- `#[proc_macro]` entry (`go!`)

## How it works

1. User writes: `go! { 10 + 20 }`
2. The proc-macro `go! { ... }` binds from tokens per `syn::Expr`
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
cargo test   # → 4 tests (simple_add, subtraction, multiplication, division)
cargo run -p gourd  # → demo binary output
cargo expand -p gourd  # → see expanded Go → Rust transpilation.
```

Original test library project 0.1.0
Original test
-- license: MSRC
-- with distribution repeto infos
