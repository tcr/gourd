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
cargo test   # → 51 tests (go! transpilation verify + functional runtime tests)
cargo run -p gourd  # → demo binary output
cargo expand -p gourd  # → see expanded Go → Rust transpilation.
```

## `verify_rust_output` — compile-time transpilation verification

The `#[verify_rust_output({ expected_rust })]` attribute macro applies to any `go!` block to **assert at compile time** that the transpiled output matches the expected Rust tokens. It lives in `gourd-codegen/src/lib.rs` and delegates to `gourd_codegen_core::verify_short()`.

### Usage

```rust
use gourd_codegen::go;

// Apply the attribute BEFORE the go! block
#[verify_rust_output({
    fn go_add(n: i32) -> i32 {
        n + 1
    }
})]
go! {
    func goAdd(n int) int {
        n + 1
    }
}
```

The brace group `{ ... }` contains the **expected Rust tokens** — exactly what the transpiler should emit. If the transpiled output doesn't match, compilation fails with a `compile_error!` showing expected vs actual.

### How it works

1. The attribute macro receives the expected tokens from its brace-group input.
2. It validates the expected tokens parse as a valid `syn::File` (i.e., valid Rust). If not, a `compile_error!` is emitted immediately, so you know your expected block is syntactically broken before comparing.
3. It extracts the `go! { ... }` body from the item following the attribute.
4. It transpiles the Go body using `transpile_go()`.
5. It normalizes both expected and actual token streams (collapsing whitespace, normalizing `::` paths).
6. If normalized tokens match → compilation proceeds (the original `go!` input is passed through).
7. If they differ → a `compile_error!` is emitted with the expected and actual token lists.

### Important details

- The proc-macro **normalizes tokens** for comparison, so you should write the expected output using standard Rust syntax and the normalizer handles whitespace/path normalization.
- The expected block **must be valid Rust syntax**. If it doesn't parse as `syn::File`, a `compile_error!` is emitted before comparison, so you get a clear error about invalid Rust rather than a confusing mismatch.
- Go-style statement separators (`;`) appear in the actual output from the transpiler. These separators are Go-to-Rust translation artifacts — the expected output must include them to match.
- Paths like `String::from` may normalize to `::std::string::String::from` in the actual output. The expected block must use the same form.
- If the expected tokens are empty (e.g., `#[verify_rust_output({})]`), verification is skipped and the block passes through unmodified — use this to get compile errors for a specific block without breaking the build.

### Common gotchas when writing expected output

| Pitfall | Fix |
|---------|-----|
| Missing `; ;` Go-style separators | Add double semicolons where the transpiler emits them (between statements) |
| `String::from(...)` vs `::std::string::String::from(...)` | Use the fully-qualified form in expected output |
| `vec![1,2,3]` vs `vec ! [ 1 , 2 , 3 ]` | The normalizer handles this — just write normal Rust |
| Multiple mismatch errors on compile | Fix one block at a time; errors are independent per block |

### Pattern for adding verify to new `go!` blocks

1. Add `use gourd_codegen::{go, verify_rust_output};` to the test file.
2. Place `#[verify_rust_output({ /* dummy */ })]` above the `go!` block.
3. Run `cargo test` — the dummy (being a comment) produces empty expected tokens, so verification is skipped. This lets you check the block compiles and runs functionally.
4. Replace `/* dummy */` with `VERIFY_MISMATCH` (a single identifier that will never match).
5. Run `cargo test` — the mismatch error shows the actual transpiled output in the `actual:` line.
6. Copy the actual output back as the expected tokens (rewriting it in readable Rust form).
7. Re-run `cargo test` — if it compiles, the verify passes.

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
