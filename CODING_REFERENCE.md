# CODING_REFERENCE — gourd (Go → Rust Transpiler)

Useful debugging patterns, Rust syntax gotchas, and architectural notes
gathered from implementing Go → Rust transpilation.

---

## Cross-Language Validation Pattern

When adding a new language mode or validating transpiled output, use the
**`gourd-check` standalone CLI** instead of trying to validate inside the
`proc_macro`. This bypasses the `proc_macro` token stream limitation
e entirely and gives access to exact source text.

### Architecture

```
gourd-check/
  src/
    scanner.rs   # Brace-matching scanner (go! { ... } extraction)
    validator.rs # go build / cargo check in temp dirs
    report.rs    # Formatted error output
    main.rs      # CLI (clap)
```

### Why not validate in the proc macro?

| Issue | Proc Macro | gourd-check |
|-------|-----------|-------------|
| Source access | `TokenStream` only (no raw text) | Raw `.rs` files |
| Formatting | `quote!` injects spaces (`func hello ( ) int`) | Preserved exactly |
| Temp isolation | Shared mutable state | Per-block `tempfile::tempdir()` |
| Errors | Custom error messages | Real compiler output |

### When to use

- Adding Go mode support (e.g. new type mappings)
- Adding a new target language (e.g. Go → Python)
- Validating that transpiled output is syntactically valid Rust
- Pre-commit checks on test files

### How to use

```bash
# Scan a single file
./target/debug/gourd-check gourd-codegen/tests/go_fn.rs

# Scan all tests
./target/debug/gourd-check gourd-codegen/tests/

# Verbose: show extracted blocks
./target/debug/gourd-check -v 2 gourd-codegen/tests/go_fn.rs

# Go-only mode
./target/debug/gourd-check -g gourd-codegen/tests/
```

### The `gourd-check` workflow for new language features

1. **Write test files** with the new language syntax inside `go! { ... }` blocks.
2. **Run `gourd-check`** to validate: `./target/debug/gourd-check -v 2 tests/`
3. **Iterate** on the transpiler until all blocks pass validation.
4. **Add `verify_rust_output`** attributes to pin expected output.
5. **Commit** — the proc macro compiles, the validator passes.

### Example: Adding a new Go type

1. Write a test: `go! { func foo(x int64) int64 { x } }`
2. Run `gourd-check -v 2` — errors show Go validates OK.
3. Run `cargo test` — check that the transpiler emits valid Rust.
4. Add `#[verify_rust_output({ fn go_foo(x: i64) -> i64 { x } })]`.
5. Re-run `cargo test` — verification passes.

---

### Known unimplemented Go forms

The following Go constructs are NOT yet transpiled — they are commented out in `gourd-codegen/tests/go_fn.rs` with explanatory notes:

| Form | Go example | Status |
|------|-----------|--------|
| Multi-return | `func foo() (int, string)` | `compile_error!` — comma-separated return list not parsed |
| Slice literals | `[]int{1, 2, 3}` | `vec![1, 2, 3]` — detected via `[]` bracket marker in `GoFnOutput::parse` + `Expr::Macro` dispatch |
| Map literals | `map[int]string{1: "one"}` | `compile_error!` — map literal syntax not implemented |
| Struct definitions | `struct Foo { x int }` | Requires non-declaration ordering in temp file |

**Fixed in recent commits:**
- ✅ Control flow (`if`/`else`) — added `GoIf` variant, `parse_block_stmts`
- ✅ Type conversions (`int()`, `uint()`, `float32()`, `float64()`, `bool()`, `byte()`, `rune()`, `string()`)

When adding new Go constructs, check which category they fall into:
- **Parser missing**: Add to `GoStmt` enum in `parsing.rs` (e.g., `for`, `switch` already implemented)
- **Transpiler missing**: Add to the relevant `go_to_rust_*` function in `free_fn.rs`
- **Type mapping missing**: Add to `map_go_types` in `types.rs`

---

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

---

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

---

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

```
gourd/
  gourd-codegen/       ← proc-macro library (transpiler core)
  gourd/               ← runtime + demo binary
```

Key files:

| File | Purpose |
|------|---------|
| `gourd-codegen/src/lib.rs` | `go!` proc-macro, dispatch logic |
| `gourd-codegen/src/transpiler.rs` | Go → Rust transpiler (~650 lines) |
| `gourd-codegen/tests/receiver_tests.rs` | Receiver scope tests |

Types in `gourd-codegen/src/transpiler/` (split across files):

### `parsing.rs` — AST types for Go declarations

| Type | Purpose |
|------|---------|
| `GoStruct` | `struct Name { field type }` → `struct Name { pub field: Type }` |
| `GoStructField` | Individual struct field: `{ name, ty }` |
| `Receiver` | `(f Foo)` or `(f *Foo)` → `name, ty, pointer` |
| `ReceiverFn` | `(receiver) name(params) output { body }` |
| `GoStmt` | `Expr(Expr)` — parsed statement |
| `GoParam` | `{ id, ty, slice_elem }` |
| `GoFnOutput` | Return type(s) as `Vec<syn::Type>` |
| `GoFnInputs` | Parsed parameters with Go-style grouping |
| `GoFn` | Top-level function: `{ ident, generics, inputs, output, block }` |
| `Switch` | `switch selector { case N: ... }` → `match selector { ... }` |
| `SwitchCase` | `{ exprs, stmts }` — case expression list + body |

### `free_fn.rs` — TokenStream entry points

| Type | Purpose |
|------|---------|
| `GoStruct` | `struct Name { field type }` → `struct Name { pub field: Type }` |
| `GoStructField` | Individual struct field: `{ name, ty }` |
| `Receiver` | `(f Foo)` or `(f *Foo)` → `name, ty, pointer` |
| `ReceiverFn` | `(receiver) name(params) output { body }` |
| `GoStmt` | `Expr(Expr)` — parsed statement |
| `GoParam` | `{ id, ty, slice_elem }` |
| `GoFnOutput` | Return type(s) as `Vec<syn::Type>` |
| `GoFnInputs` | Parsed parameters with Go-style grouping |
| `GoFn` | Top-level function: `{ ident, generics, inputs, output, block }` |

Key functions:

| Function | File | Purpose |
|----------|------|---------|
| `go_stmt_to_rust` | `parsing.rs` | Dispatch parsed `GoStmt` variants → Rust |
| `go_to_rust_struct` | `free_fn.rs` | Struct decl → Rust struct |
| `go_to_rust_receiver_fn` | `funcs.rs` | Receiver fn → impl block |
| `go_to_rust_fn` | `free_fn.rs` | Free function declaration |
| `go_to_rust_switch` | `free_fn.rs` | Switch decl → Rust match |
| `transpile_switch` | `free_fn.rs` | `Switch` AST → `match` expression |
| `Receiver::from_tokens` | `funcs.rs` | Parse `(name Type)` / `(name *Type)` |
| `ReceiverFn::parse` | `funcs.rs` | Full receiver function parsing |
| `replace_receiver` | `funcs.rs` | Rename receiver ident → `self` |

---

## Go Struct ↔ Rust Struct

| Go | Rust |
|----|------|
| `struct Foo { x int }` | `struct Foo { pub x: i32 }` |
| `struct Bar { name string, count int }` | `struct Bar { pub name: String, pub count: i32 }` |

Fields are automatically made `pub`.

## Go Receiver Function ↔ Rust impl block

| Go | Rust |
|----|------|
| `func (f Foo) Bar() int { f.x }` | `impl Foo { fn Bar(&self) -> i32 { self.x } }` |
| `func (f *Foo) Baz(z int) int { f.x = f.x + z; f.x }` | `impl Foo { fn Baz(&mut self, z: i32) -> i32 { self.x = self.x + z; self.x } }` |

Value receiver (no `*`) → `&self`. Pointer receiver (`*`) → `&mut self`.

## Go Type Map

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

## Slices

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

## Go Switch ↔ Rust Match

| Go | Rust |
|----|------|
| `switch n { case 1: "one" case 2: "two" default: "other" }` | `match n { 1 => "one", 2 => "two", _ => "other" }` |

Switch selectors are parsed as `Path` (not `Expr`) to avoid `x { }` being
consumed as verbatim. Multiple case expressions become comma-separated
match patterns.
