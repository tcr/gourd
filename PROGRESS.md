# Gourd: Progress on Map & Slice Literals

## What I've Done (Round 2)

Resolved all compilation errors. Below is a chronological log of fixes and findings.

### Fix 1: `Delimiter::Curly` → `Delimiter::Brace` (6 occurrences)

`proc-macro2` v1.0.106 *does* have a brace variant — it's called **`Brace`**, not `Curly` or `CurlyBrace`. All references were replaced:

| Line | Before | After |
|------|--------|-------|
| 1265 | `Delimiter::Curly` | `Delimiter::Brace` |
| 1315 | `Delimiter::Curly` | `Delimiter::Brace` |
| 1330 | `Delimiter::Curly` | `Delimiter::Brace` |
| 1367 | `Delimiter::Curly` | `Delimiter::Brace` |
| 1386 | `Delimiter::Curly` | `Delimiter::Brace` |
| (lib.rs) | imports: `Delimiter` already correct | — |

**Key insight:** The variant name is `Brace`, matching `proc_macro2::Delimiter::Brace`. There is no distinction between "opening" and "closing" braces — a `Group` with `Delimiter::Brace` represents the entire `{ ... }` block. Both `syn::braced!()` (parsing) and `proc_macro2::Group::new(Delimiter::Brace, ...)` (construction) use the same variant.

### Fix 2: Missing semicolon on line 1340

Changed `}` to `;` on line 1340. This is the closing of a block expression assigned to `let val_type: Option<syn::Type> = { ... };`. Without the semicolon, the compiler saw the next statement `let mut synthetic ...` and raised "expected `;`, found keyword `let`".

### Fix 3: Map literal transpilation — `.insert()` method calls

Fixed `go_to_rust_map` (line 1221): changed `.insert(#key, #val)` (method-chain style that is invalid as a standalone statement) to `m.insert(#key, #val);` (complete statement with explicit receiver). The original produced tokens like `.insert(a,1)` which Rust cannot parse as a statement — the error "expected expression, found `.`" was a symptom.

### Fix 4: Go string literals → Rust `String`

Updated `transpile_lit` to convert Go string literals (`"hello"`) to Rust `std::string::String::from("hello")` instead of passing them through as-is (`"hello"` → `&str`). Go strings are always owned UTF-8 values; Rust `"..."` is a borrowed `&str`. This matters for map literals where keys/values are `HashMap<String, T>` — previously `"a"` would infer as `&str` and cause type mismatches.

## What I've Learned (Round 3)

### The delimiter mapping is straightforward

`proc_macro2::Delimiter` actually has three variants that map directly:

| proc_macro2 | Usage |
|-------------|-------|
| `Delimiter::Parenthesis` | `( ... )` |
| `Delimiter::Brace` | `{ ... }` |
| `Delimiter::Bracket` | `[ ... ]` |

There is *also* `Delimiter::None` which represents invisible/emphatic delimiters (used by macro variables). This was the confusion point: `proc_macro::Delimiter::CurlyBrace` (in the real `proc_macro` crate) maps to `proc_macro2::Delimiter::Brace` (not `None`). The old notes were wrong — it's a simple rename.

### The reconstruction strategy works

`parse_go_slice` and `parse_go_map` extract inner tokens from existing `Group(Bracket, ...)` and `Group(Brace, ...)` tokens, construct a synthetic `TokenStream` from them, and call `syn::parse2::<GoSliceLit>()` / `syn::parse2::<GoMapLit>()`. This approach succeeds because `syn::braced!()` and `syn::bracketed!()` macros handle `proc_macro2::TokenTree::Group` tokens correctly when the delimiter matches the expected type.

### Map literals now work end-to-end

Test results (all 59 tests pass across 8 test files):

| Test file | Status |
|-----------|--------|
| `coverage.rs` (23 tests) | ✅ all pass |
| `slice_map_debug.rs` (6 tests) | ✅ all pass |
| `embed_tests.rs` (4 tests) | ✅ all pass |
| `go_fn.rs` (13 tests) | ✅ all pass |
| `receiver_tests.rs` (3 tests) | ✅ all pass |
| `shorthand_query.rs` (2 tests) | ✅ all pass |
| `gc_tests.rs` (8 tests) | ✅ all pass |

Verified slice/map literal parsing for:
- `[]int{ 1, 2, 3 }` ✅
- `[]int{ }` (empty) ✅  
- `[]{ 10, 20, 30, 40 }` (inferred) ✅
- `map[string]int{ "a": 1, "b": 2, "c": 3 }` ✅
- `map[string]int{ }` (empty) ✅
- `map[int]string{ 1: "one", 2: "two" }` ✅

## What I'm Trying Next

1. **Add debugging infrastructure** — use `cargo expand -p gourd-codegen` to visually verify the expanded form is emitted for slice/map literals, then expand and diff against expected Rust output.

2. **Consider type-aware transpilation** for slices — when Go code contains `[]string{ "a", "b" }`, the transpiler currently outputs `vec![ String::from("a"), String::from("b") ]` which is correct but infers `Vec<String>`. Should explore whether the element-level transpiler can introspect whether it's inside a slice literal context to produce context-appropriate output. (Currently all Go string literals already produce `String::from(...)`, so this works.)

3. **Handle composite Go literals** — Go supports composite literals like `map[string]int{}` inside nested structures (e.g., as a field value in a struct literal). Need to extend the `Expr::Verbatim` fallback path (line 40-51) to handle nested slice/map constructions.

4. **Add `go!` macro support** for composite types: `struct User { name string, tags []string }` with field values being slice/map literals in the body.
