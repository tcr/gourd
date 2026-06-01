# Gourd

Transpiles inline Go declarations into valid Rust via a procedural macro at compile time.

```
gourd/
  gourd-codegen/       <-- proc-macro library (transpiler core)
  gourd/               <-- runtime + CLI tool (`gourd transpile`)
```

[`gourd-codegen/src/transpiler.rs`]  -- Go → Rust transpiler
[`gourd-codegen/src/lib.rs`]         -- `#[proc_macro]` entry (`go!`)
[`gourd/src/main.rs`]              -- CLI tool (`gourd transpile`)

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

### Known unimplemented Go forms

The following Go constructs are NOT yet transpiled — they are commented out in test files with explanatory notes:

| Form | Example | Status |
|------|---------|--------|
| Slice literals | `[]int{1, 2, 3}` | ✅ Implemented |
| Map literals | `map[int]string{1: "one"}` | ✅ Implemented |
| Multi-return | `func foo() (int, string)` | ✅ Implemented |
| Struct definitions | `struct Foo { x int }` | ✅ Implemented |
| Map access | `m[key]` | ✅ Implemented |
| Continue statement | `continue` | ✅ Implemented |
| While loops | `while cond { ... }` | ✅ Implemented |
| For range | `for i, v := range data` | ✅ Implemented |
| Concurrency | `go func()`, `chan`, `select` | Not implemented |
| Interfaces | `interface{}` | ✅ Implemented |

### Recently fixed
- ✅ Interfaces — `interface Name { Method() Type }` → `trait name { fn method(&self) -> Type; }`
- ✅ Control flow (`if`/`else` statements)
- ✅ Type conversions (`int()`, `uint()`, `float32()`, `float64()`, `bool()`, `byte()`, `rune()`, `string()`)
- ✅ Semicolon insertion in Go validation harness
- ✅ Multi-return values (`return a, b` → `return (a, b)`)
- ✅ Map literals (`map[string]int{"a": 1}` → `HashMap::new(); m.insert(...)`)
- ✅ Slice literals — `[]int{1, 2, 3}` produces `vec![1, 2, 3]`
- ✅ While loops — `while cond { ... }` → `while cond { ... }`
- ✅ Continue statements — `continue` → `continue`
- ✅ For range loops — `for i, v := range data` → `for (i, v) in data.iter().copied().enumerate()`
- ✅ Nested `if`/`continue`/`while` — proper block body parsing in `parse_go_if` and `parse_block_stmts`

## Running

```bash
cargo test   # → 50 tests (go! transpilation verify + functional runtime tests + gourd-check)
gourd transpile "func hello() int { return 42 }"  # → transpile CLI tool
cargo expand -p gourd  # → see expanded Go → Rust transpilation.
gourd-check [PATHS...]      # Standalone Go/Rust validation (same scanner + validators)
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

### Writing correct `#[verify_rust_output]` attributes

- **Function names**: The transpiler converts Go names to Rust snake_case via `to_snake_case`. Handle trailing digits: `goShorthand2` → `go_shorthand_2`, `goAdd` → `go_add`, `isEven` → `is_even`.
- **Return statements**: The transpiler always adds explicit `return` before expressions. Expected: `{ return a + b }`, not `{ a + b }`.
- **Method calls on string/slice**: `len(s)` in Go becomes `s.len() as i32` in Rust (type conversion is wrapped in `int(...)` in (now fully handled — see "Type conversions" section above).
- **String literals**: `"hello"` in Go becomes `::std::string::String::from("hello")` in Rust.
- **Slice/map types**: `[]int` becomes `&[i32]`, slice literals `[]int{...}` inside function bodies ARE transpiled (produce `vec![...]`). Go slice type `[]int` in function signatures is detected via `[]` bracket marker in `GoFnOutput::parse` and the element type is stored for `Vec<i32>` return generation.

## Cross-Language Validation Pattern (the `gourd-check` pattern)

When adding a new language mode or validating transpiled output, **always use `gourd-check`** as the standalone pre-compilation validator. This pattern bypasses the `proc_macro` token stream limitation entirely by operating directly on raw source files.

### When to use

- Adding a new language mode (e.g. Go → a new target language)
- Validating Go syntax inside `go! { ... }` blocks
- Validating transpiled Rust output for correctness
- Building any feature that requires source-level code inspection before macro expansion

### How it works

```
Source file (.rs)
    │
    ▼
[scanner.rs] Brace-matching → extracts go! { ... } raw text
    │
    ▼
[validator.rs] Writes temp harness + runs real compiler
    │                    │
    │              go build  (for Go code)
    │              cargo check (for Rust code)
    │
    ▼
[report.rs]   Formats errors: file:line, message, code excerpt
```

### Key design decisions

1. **Pre-macro expansion**: Operates on source files directly, not `proc_macro::TokenStream`. This gives access to exact formatting — no `quote!` spacing artifacts.
2. **Brace-matching, not regex**: Recursively tracks brace depth to correctly extract nested `go! { if { ... } }` blocks.
3. **Per-block temp dirs**: Each block validated in its own `tempfile::tempdir()` to avoid file conflicts.
4. **Real compilers only**: Uses `go build` and `cargo check` — no custom parsers. Errors are always accurate.

### Usage

```bash
gourd-check [PATHS...]       # Scan files (default: current directory)
gourd-check -g PATHS         # Go-only validation
gourd-check -r PATHS         # Rust-only validation
gourd-check -v 2 PATHS       # Verbose: show block details
gourd-check --help           # Help
```

> **Note**: Running `cargo test` at the workspace root automatically runs `gourd-check` — Go blocks are validated via `go build` and `#[verify_rust_output]` blocks are validated via `cargo check`.

### Example output

```
gourd-codegen/tests/go_fn.rs:21
    Go: main.go:14:9: a + b (value of type int) is not used
         main.go:15:5: missing return
    1 | func goSum(a int, b int) int {
    2 |         a + b
    3 |     }
```

### When the proc macro IS appropriate

Use `proc_macro` only for the actual **transpilation** — when you need to transform tokens at compile time in the user's crate. Use `gourd-check` for **validation** — checking that extracted source text is syntactically/semantically correct.

### Pitfalls

| Pitfall | Solution |
|---------|----------|
| `proc_macro::TokenStream::to_string()` loses formatting | Use `gourd-check` — it reads raw source files |
| Temp dir file conflicts between blocks | Use per-block temp dirs (`tempfile::tempdir()`) |
| `quote!` spacing (`func hello ( ) int`) breaks Go parser | Don't use `quote!` for validation — use raw source text |
| `compile_error!` inside macro items requires `;` | Emit `compile_error!` with proper semicolons |

## Working with files

- If a file is over 400 lines long, consider breaking it into multiple files. 
- Please lean on the `rust-analyzer` MCP for refactoring and inspecting Rust types. The Rust Analyzer MCP is much better at refactoring than copy/paste. It also is useful for navigating the codebase.
- For other edits, consider using command line tools like `cp` and `sed` to work exactly with line numbers. Whenever trying to recover a misedited file, attempt to read its previous contents from `git`.

## Name Mapping: `to_snake_case`

Location: `gourd-codegen-core/src/transpiler/free_fn.rs`

Converts Go function/variable names to Rust snake_case. Key behaviors:

| Go name | Rust name | Notes |
|---------|-----------|-------|
| `goAdd` | `go_add` | Standard camelCase → snake_case |
| `goShorthand2` | `go_shorthand_2` | Trailing digits after lowercase get `_` prefix |
| `isEven` | `is_even` | No `go_` prefix in Go name → no prefix in Rust |
| `go_is_even` | `go_is_even` | Already snake_case, no transformation |
| `hello` | `hello` | All lowercase, no change |

The function adds underscores before:
- Uppercase letters (if not preceded by `_`)
- ASCII digits if preceded by a lowercase letter (e.g., `Shorthand2` → `shorthand_2`)

When writing `#[verify_rust_output]` attributes, the expected function name must match this conversion.

## Useful Patterns

### Speculative Parsing (fork/advance)

Use `input.fork()` to peek ahead without consuming input. If parsing
succeeds, call `input.advance_to(&fork)` to commit. If it fails, the
original cursor is untouched — just try the next alternative.

```rust
let fork = input.fork();
match fork.parse::<SomeType>() {
    Ok(_) => {
        input.advance_to(&fork);
        // committed — use `input` from here
    }
    Err(_) => {
        // cursor unchanged — try a different parser
    }
}
```

This is how `GoFnInputs::parse` distinguishes Go-style grouped parameters
(`a, b, c int`) from separate parameters (`a int, b int`).

### Emitting compile-time errors for unsupported forms

```rust
fn emit_todo(msg: &'static str) -> TokenStream {
    quote! { {
        compile_error!(concat!("TODO: ", #msg))
    }
    unreachable!()  }}
}
```

Use this when a Go construct has no Rust equivalent yet — the macro
expands to a compile error that tells the user (via `cargo build`)
exactly which feature is missing.

### Using `quote!` for debugging

`syn::Type` does NOT implement `Debug` or `Display`. To inspect one:

```rust
let debug_str = quote! { #some_type }.to_string();
```

This is `transpiler.rs:464` technique — necessary for printing type info
during macro expansion.

### Using `quote!` to inject TokenStreams

`proc_macro2::TokenStream` DOES implement `ToTokens`. Use this to
inject a lazily-built snippet:

```rust
let snippet: TokenStream = quote! { self.x + z };
quote! { {} #snippet };
```

## Rust Syntax Gotchas (syn / Rust)

### Switch expressions → match patterns vs body expressions

Go `switch` statements can have two roles for their case expressions:

1. **Match patterns** (selector present): `case 1, 2: "one_or_two"`
2. **Boolean conditions** (no selector): `case ok: "ok"`

For **match patterns**, use `go_to_rust_pattern()` which keeps string
literals as `&str` patterns (`"..."`) instead of wrapping them in
`String::from(...)`. Rust match arms require patterns, not expressions.

For **body expressions**, use `go_to_rust()` which wraps strings in
`::std::string::String::from(...)` to satisfy type requirements.

```rust
// Pattern case expressions: go_to_rust_pattern(expr)
// Body expressions: go_to_rust(expr)
// Selector: just the identifier via go_to_rust(switch.selector)
```

### No-selector switch → if-else chain

A Go `switch` without a selector (`switch { case ok: ... }`) has
no selector expression — case expressions are boolean conditions.
Transpile to a connected `if/else if/else` chain:

```rust
if first_cond { body } else if second_cond { body } else { default }
```

Build by:
1. First case → initial `if`
2. Subsequent cases → `else if`
3. Default → final `else`

The whole chain must be a **single expression** that returns the body
value — do NOT emit independent `if` blocks.

### `syn::Type` does NOT implement `Debug` or `Display`

```rust
let ty: syn::Type = syn::parse_str("i32").unwrap();
// println!("{:?}", ty);       // COMPILE ERROR — no Debug impl
// println!("{}", ty);          // COMPILE ERROR — no Display impl
println!("{}", quote! { #ty }); // ✅ works
```

*Workaround:* Use `quote! { #ty }.to_string()` for debugging / logging.

### `syn::Local` does NOT implement `Parse`

You cannot do `input.parse::<syn::Local>()`. The `Local` struct lacks the
`Parse` impl. Instead, parse `syn::Expr` (which covers `Expr::Let` for
Go's `:=` operator):

```rust
// WRONG:
let local: syn::Local = input.parse()?;

// RIGHT:
let expr: syn::Expr = input.parse()?;
// Expr::Let → Go short declaration `x := y`
// Expr::Assign → Go assignment `x = y`
```

### `syn::LocalInit` fields are `eq_token` and `expr`

```rust
// The init of a `let` statement:
syn::LocalInit {
    eq_token: Token![=](_),
    expr: Box<Expr>,
    diverge: Brace(_),  // optional
}
```

Not `base` or `assign_token` — those names are wrong/old.

### Field and Path accessors have extra fields

`ExprField` has `attrs: Vec<Attribute>`.
`ExprPath` has `qself: Option<QSelf>` (for `<T as Trait>::Method`).

When manually constructing these (e.g. in `replace_receiver`), you MUST
provide them:

```rust
ExprField {
    attrs: Vec::new(),
    base: Box::new(…),
    dot_token: Token![.](…),
    member: Member::Name(Ident::new("x", …)),
}
```

### `Expr::parse` on `name { }` consumes the whole thing as verbatim

When the input stream has an identifier followed by a brace group —
like `x { }` — `input.parse::<Expr>()` will NOT parse just `x` and
leave `{ }` in the stream. Instead, syn captures the entire `x { }`
as `Expr::Verbatim` (it's not a valid Rust expression, so syn falls
back to verbatim).

This breaks subsequent `syn::braced!()` calls because the braces are
gone. The fix: parse just a `Path` instead of `Expr` for identifiers
that precede braces:

```rust
// WRONG — consumes `x { }` as Expr::Verbatim
let selector: Expr = input.parse()?;

// RIGHT — stops at `{` boundary
let path: syn::Path = input.parse()?;
Some(syn::Expr::Path(syn::ExprPath {
    attrs: Vec::new(),
    qself: None,
    path,
}))
```

### `syn::parenthesized!` must be used with `in input` syntax

```rust
let content;
let _paren = syn::parenthesized!(content in input);
// content is a ParseStream — call `.parse::<Type>()` on it
```

Not: `parenthesized!(input)` — that does not exist and will not compile.
The return type is `proc_macro2::GroupName`, specifically
`proc_macro2:: groups::Parenthesis`.

### `proc_macro2::Delimiter` variant names

| `proc_macro2` | Usage |
|---------------|-------|
| `Delimiter::Parenthesis` | `( ... )` |
| `Delimiter::Brace` | `{ ... }` |
| `Delimiter::Bracket` | `[ ... ]` |
| `Delimiter::None` | invisible (macro variables) |

The variant name is `Brace`, matching `proc_macro2::Delimiter::Brace`. There is *no* `Curly` or `CurlyBrace` variant — the old `Delimiter::Curly` (from `proc_macro` v0.2.x) was renamed to `Brace` in `proc_macro2` v1.0. Both `syn::braced!()` (parsing) and `proc_macro2::Group::new(Delimiter::Brace, ...)` (construction) use the same delimiter.

### Method-chain `.insert(key, val)` is not a statement

Syntax `.insert(a, 1)` is not a valid standalone Rust statement (error: "expected expression, found `.`"). Always provide the explicit receiver:

```rust
// WRONG.
m.insert(a, 1)  // ❌ — syntax error: expected expression, found `.`

// RIGHT.
m.insert(a, 1);  // ✅
```

### `Token![;]` is `syn::token::Semi`, not `Token`

```rust
use syn::token;
input.peek(token::Semi)        // ✅ true if next is `;`
let _semi: token::Semi = input.parse()?;  // ✅ consume it
```

The syntax `Token![;]` is deprecated / does not compile.

### Reserved keywords in Go (`switch`, `case`, `default`) via `Ident::parse_any`

Rust keywords like `switch`, `case`, and `default` are valid identifiers
in Go but reserved in Rust. Use `Ident::parse_any` (from `syn::ext::IdentExt`)
which treats reserved keywords as identifiers:

```rust
use syn::ext::IdentExt;
let kw: Ident = input.call(Ident::parse_any)?;  // parses "switch" etc.
let kw_str = kw.to_string();
if kw_str == "case" { /* ... */ }
```

### Case parsing: colon delimiter with speculative expression parsing

Go case lines look like `case 1, 2, 3:` — comma-separated expressions
terminated by a colon. Parse them with a fork-to-colon loop:

```rust
loop {
    if brace_content.peek(syn::token::Colon) {
        break;  // reached `:` — stop
    }
    let expr: Expr = brace_content.parse()?;
    exprs.push(expr);
    if brace_content.peek(syn::token::Comma) {
        let _: syn::token::Comma = brace_content.parse()?;
    } else {
        break;
    }
}
let _: syn::token::Colon = brace_content.parse()?;  // consume `:`
```

### `*Type` is NOT a valid Rust type string

`syn::parse_str::<syn::Type>("*Foo")` fails because `*Foo` is not valid
Rust text. You need `*const Foo` or `*mut Foo`. This matters when parsing
Go pointer receivers: extract the `*` separately, then parse the bare type.

### `syn::parse_str::<syn::Type>("unknown")` returns `Ok`

Parsing an unknown identifier name as a type does NOT fail:

```rust
// Returns: Ok(Type::Path { path: Path { segments: ["unknown"] } })
syn::parse_str::<syn::Type>("unknown")
```

This is useful for fallback/empty cases — "unknown" resolves to a valid
type path you can clap in `else` branches.

### `syn::Type::Reference` (Go `&T`) maps to Rust references

```rust
syn::Type::Reference { elem: Box<Type>, lifetime: Option<Lifetime> }
// Map: &T → &MappedT, &lifetime T → &'lifetime MappedT
```

### `syn::parse_quote!` is your friend

```rust
// Instead of writing:
// Block { stmts: vec![Stmt::Expr(…), …] }
// Use:
let body: Box<Block> = syn::parse_quote! { { statements; here; } };
```

It parses text → syn AST. Essential for constructing AST nodes from
strings without implementing `Parse` manually.

### `quote!` with a `String` produces a string literal, NOT an identifier

This was a subtle but critical bug. When interpolating a `String` into
`quote!`, the string content becomes a Rust string literal (`"m"`), not
an identifier token:

```rust
let ident = String::from("m");
quote! { let #ident = 42; }  // ❌ → `let "m" = 42;` (string literal!)

let name: syn::Ident = syn::parse_str(&ident).unwrap();
quote! { let #name = 42; }   // ✅ → `let m = 42;` (proper identifier)
```

**Rule:** Always convert a `String` to `syn::Ident` via `syn::parse_str`
before interpolating into `quote!` as a name.

### `HashMap::new()` without type hints causes inference failure

```rust
let mut m = std::collections::HashMap::new();
m.is_empty();  // ❌ cannot infer HashMap<K, V>
```

For empty maps, use `HashMap::default()` which defers type resolution:
```rust
let mut m = std::collections::HashMap::default();
```

For non-empty maps, provide type parameters:
```rust
let mut m = std::collections::HashMap::<i32, String>::new();
```

### `HashMap::get(key)` requires a reference: `get(&key)`

Rust's `HashMap::get(&Q)` takes a reference to the key type. Go code like
`m[2]` or `m.get(2)` in the Go block must be translated to `m.get(&2)`:

```rust
fn transpile_method_call(input: &ExprMethodCall) -> TokenStream {
    // For `.get(key)`, wrap key in &
    if method_name == "get" {
        let args = args.iter().enumerate().map(|(i, a)| {
            if i == 0 { quote! { &#a } } else { quote! { #a } }
        });
        return quote! { #receiver.#method_name( #(#args),* ) };
    }
    // ... normal case
}
```

### `syn::braced!` fails on nested groups — use `step()` instead

`syn::braced!(content in input)` does NOT work when `input` is nested
inside another brace-delimited group. The workaround is to use a
`step()` closure to extract the group content from the parent cursor:

```rust
let content = cursor.step(|cursor| {
    if let Some((inner, _, rest)) = cursor.group(proc_macro2::Delimiter::Brace) {
        Ok((inner.token_stream(), rest))
    } else {
        Err(cursor.error("expected `{`"))
    }
});
```

### Early return with `?` prevents fallback parsing

When trying to parse `Stmt` from a block and then falling back to
alternate logic (e.g., `let` statement detection), using `?` propagates
errors and exits. Replace `?` with `if let Ok(...) =` to continue:

```rust
// WRONG — exits on failure, never reaches fallback:
brace_content.parse::<Stmt>()?;
fallback_logic();

// RIGHT — falls through on failure:
if let Ok(stmt) = brace_content.parse::<Stmt>() {
    // parsed successfully
} else {
    fallback_logic();  // tries alternate parsing
}
```

### Capturing map key/value types from Go declarations

When parsing `map[K]V{entries}`, capture the key and value types:

```rust
// Parse K from [K]
let k_content;
let _ = syn::bracketed!(k_content in input);
let key_type = if !k_content.is_empty() {
    k_content.parse::<syn::Type>().ok()
} else { None };

// Parse V
let val_type = if input.peek(syn::Ident) || input.peek(syn::token::Bracket) {
    input.parse::<syn::Type>().ok()
} else { None };

// Map Go types → Rust types via `map_go_types(key_type)?`
```

### `Cursor` lacks `parse()`/`peek()` methods

`proc_macro2::Cursor` (from `step()` closures) does NOT have `.parse()`
or `.peek()` methods. To inspect a cursor's contents, convert it to a
`TokenStream` first:

```rust
cursor.step(|cursor| {
    let ts: TokenStream = cursor.token_stream();
    let result = syn::parse2::<syn::Type>(ts);  // ✅ works
    Ok((result, cursor))
});
```

## Type Mappings

### Go Type Map

| Go | Rust |
|----|------|
| `int` | `i32` |
| `int8` | `i8` |
| `int16` | `i16` |
| `int32` | `i32` |
| `int64` | `i64` |
| `uint` | `u32` |
| `uint8` | `u8` |
| `uint16` | `u16` |
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

### Go Struct ↔ Rust Struct

| Go | Rust |
|----|------|
| `struct Foo { x int }` | `struct Foo { pub x: i32 }` |
| `struct Bar { name string, count int }` | `struct Bar { pub name: String, pub count: i32 }` |

Fields are automatically made `pub`.

### Go Receiver Function ↔ Rust impl block

| Go | Rust |
|----|------|
| `func (f Foo) Bar() int { f.x }` | `impl Foo { fn Bar(&self) -> i32 { self.x } }` |
| `func (f *Foo) Baz(z int) int { f.x = f.x + z; f.x }` | `impl Foo { fn Baz(&mut self, z: i32) -> i32 { self.x = self.x + z; self.x } }` |

Value receiver (no `*`) → `&self`. Pointer receiver (`*`) → `&mut self`.

### Slices

Go `[]T` is not a valid Rust slice type. This is mapped to the fixed-size
array slice pointer type `&[T]` instead.

| Go | Rust |
|----|------|
| `[]int` | `&[i32]` |
| `a []int` | `a: &[i32]` |
| `[]int{1, 2, 3}` (literal) | `vec![1, 2, 3]` |

Slice type detection works via a `[]` bracket marker in `GoFnOutput::parse`.
When a `[]T` return type is encountered, the element type `T` is stored in
`GoFnOutput.elem_type` (e.g., `int` → element type for `Vec<i32>`). In
function bodies, `return []int{1, 2, 3}` is handled by the slice literal
handler in `parse_go_block` which extracts elements from the brace group
and generates `vec![1, 2, 3]`. The `Expr::Macro` dispatch in `go_to_rust`
ensures macro invocations like `vec!` pass through correctly.

### Go Switch ↔ Rust Match

| Go | Rust |
|----|------|
| `switch n { case 1: "one" case 2: "two" default: "other" }` | `match n { 1 => "one", 2 => "two", _ => "other" }` |

Switch selectors are parsed as `Path` (not `Expr`) to avoid `x { }` being
consumed as verbatim. Multiple case expressions become comma-separated
match patterns.

## Development Instructions

You are encouraged to add debug logs and diagnostics and try re-running the program as often as you like. This is a toy repository. You will often have more success implementing and reading debug statements and running cargo expand than by reading the code.

But do try keeping changes small, iterative, and working toward finishing the implementation.
