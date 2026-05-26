# Gourd

Transpiles inline Go expressions into valid Rust via a procedural macro at compile time.

```
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

## RFCs

RFCs are now written *after the fact* to describe an implemented feature and its design decisions. RFCs are numbered sequentially relative to other RFCs in its folder.

## Development Instructions

Please read @PROGRESS.md and then read these instructions:

You are encouraged to add debug logs and diagnostics and try re-running the program as often as you like. This is a toy repository. You will often have more success implementing and reading debug statements and running cargo expand than by reading the code.

But do try keeping changes small, iterative, and working toward finishing the implementation.
