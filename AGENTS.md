# Gourd

Transpiles inline Go declarations into valid Rust via a procedural macro at compile time.

```
gourd/
  gourd-codegen/       <-- proc-macro library (transpiler core)
  gourd/               <-- runtime + demo binary
```

[`gourd-codegen/src/transpiler.rs`]  -- Go → Rust transpiler
[`gourd-codegen/src/lib.rs`]         -- `#[proc_macro]` entry (`go!`)

## Example of how it works

1. User writes: `go! { fn hello() string { String::from("hello") } }`
2. The proc-macro `go!` inspects tokens (struct, func, fn) to dispatch to the correct handler
3. The transpiler converts Go type names, parameters, bounds, and bodies to Rust
4. Emits pure `quote! { fn hello() -> String { String::from("hello") } }`.

Every Go declaration in the source can be valid Rust tokens. The macro
emits exactly those tokens into the user's expanded AST.

### Unsupported forms → `compile_error!`

Any Go concept missing from the transpiler (e.g. concurrency, storage, streams, etc.) expands to a `compile_error!` that
reports "TODO: transpile this Go form: <name>" at compile time of the
consumer's crate.

## Running

```bash
cargo test   # → 4 tests (go! function & receiver tests)
cargo run -p gourd  # → demo binary output
cargo expand -p gourd  # → see expanded Go → Rust transpilation.
```

## RFCs

RFCs are now written *after the fact* to describe an implemented feature and its design decisions. RFCs are numbered sequentially relative to other RFCs in its folder.

## Working with files

- If a file is over 400 lines long, consider breaking it into multiple files. 
- Please lean on the `rust-analyzer` MCP for refactoring and inspecting Rust types. The Rust Analyzer MCP is much better at refactoring than copy/paste. It also is useful for navigating the codebase.
- For other edits, consider using command line tools like `cp` and `sed` to work exactly with line numbers. Whenever trying to recover a misedited file, attempt to read its previous contents from `git`.

## Development Instructions

ALWAYS read @CODING_REFERENCE.md when editing code.

NEVER use a sub-agent or task unless instructed to.

You are encouraged to add debug logs and diagnostics and try re-running the program as often as you like. This is a toy repository. You will often have more success implementing and reading debug statements and running cargo expand than by reading the code.

But do try keeping changes small, iterative, and working toward finishing the implementation.
