# CODING_REFERENCE — gourd (Go → Rust Transpiler)

Useful debugging patterns, Rust syntax gotchas, and architectural notes
gathered from implementing Go → Rust transpilation.

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

## Go Switch ↔ Rust Match

| Go | Rust |
|----|------|
| `switch n { case 1: "one" case 2: "two" default: "other" }` | `match n { 1 => "one", 2 => "two", _ => "other" }` |

Switch selectors are parsed as `Path` (not `Expr`) to avoid `x { }` being
consumed as verbatim. Multiple case expressions become comma-separated
match patterns.

---

## Eval Code Quality (File/Function Length) via MCP

Two MCP tools: `rust_analyzer_symbols` (for file contents) and
`rust_analyzer_workspace_diagnostics` (for workspace-level errors).

### Quick per-file check (terminal)

```bash
# 1. Find all source files and their line counts (excluding tests):
find gourd-codegen/src gourd/src -name '*.rs' -exec wc -l {} +

# 2. Sort by line count, longest first:
find gourd-codegen/src gourd/src -name '*.rs' -exec wc -l {} + | sort -t'/' -k9n
```

Thresholds: **500 lines** per file, **100 lines** per function.

### Per-file: `rust_analyzer_symbols` with verbose=true

```
rust_analyzer_symbols(
    file_path = "gourd-codegen/src/transpiler.rs",
    verbose = true,
    limit = 1000
)
```

This returns `symbols[].location.range` — each top-level symbol (function,
struct, impl) with `start.line` and `end.line`. Line span =
`end.line - start.line + 1`.

To find long functions within a file, look at symbols with `kind == 12`
(function). For `impl` blocks, look for `kind == 19`.

### Per-workspace: `rust_analyzer_workspace_diagnostics`

```
rust_analyzer_workspace_diagnostics(verbose = true, workspace_id = <ws_id>)
```

Returns all compiler errors, warnings, hints,
and information-level issues across every file with attached data for
Full diagnostics check:

```
# Step 1: Ensure workspace is registered (or re-add it):
rust_analyzer_add_workspace(path = "gourd")

# Step 2: Check for any compilation errors:
rust_analyzer_workspace_diagnostics(workspace_id = "ws-2", verbose = true)
```

Empty `files` / zero counts in `summary` = clean build. Non-empty =
see `files[<uri>].diagnostics`.

### Sliding inspection: `inlay_hints`

Pass `verbose = true` to `rust_analyzer_inlay_hints` on a file to get
parameter names and inferred types for every statement. Not about
length — but useful when reading long functions:

```
rust_analyzer_inlay_hints(
    file_path = "gourd-codegen/src/transpiler/funcs.rs",
    line = 0, character = 0,
    end_line = 469, end_character = 0,
    workspace_id = "ws-2"
)
```
