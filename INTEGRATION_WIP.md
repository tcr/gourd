# Integration WIP — data_filtering.go

## Goal
Run `cargo run -p gourd -- run examples/data_filtering.go` successfully.

## Status: ✅ Compiles AND correct output!

## Fixed bugs

### 1. Loop condition `!` operator precedence (CRITICAL)
**Files:** `gourd-codegen/src/transpiler/expr/operators.rs`, `gourd-codegen/src/transpiler/stmt_to_rust.rs`

**Problem:** In Go, `!i < len(words)` means `!(i < len(words))`. In Rust, `!i < words.len() as i32` means `(!i) < (words.len() as i32)` because `!` (bitwise NOT) has HIGHER precedence than `<`. For `i=0`, `!0 = -1` and `-1 < 11` is `true`, so the loop broke immediately.

**Fix:** Parenthesized the NOT expression:
- `operators.rs`: `quote! { ! #inner }` → `quote! { !(#inner) }`
- `stmt_to_rust.rs`: `if !#cond { break; }` → `if !(#cond) { break; }` (2 places)

**Result:** `Has long words: false` → `Has long words: true` ✓

### 2. `TrimAndFormat` empty output (CRITICAL)
**Root cause:** Same `!` precedence bug — the loop condition `if !i < words.len()` was causing the loop to break on the first iteration.

**Result:** `Trimmed: ` (empty) → `Trimmed: the | quick | brown | fox | jumps | over the | lazy | dog | the | fox` ✓

### 3. `fields(text)` call argument `.clone()` (MEDIUM)
**File:** `gourd-codegen/src/transpiler/expr/calls.rs`

**Fix:** Added `.clone()` for simple identifier arguments in `transpile_call` to avoid ownership moves in Go function bodies where `text` is used multiple times.

## Current output

```
Has long words: true
Top 3: {the:3, dog:1, fox:2}
Trimmed: the | quick | brown | fox | jumps | over the | lazy | dog | the | fox
Filtered: hello there
Duration: 1.500s
Greet: Hello, world!
```

## Notes

- `Top 3` output is non-deterministic because `WordFreqTopN` in the Go source returns the first N map entries without sorting by frequency. Go map iteration order is not guaranteed, so this is a bug in the Go example, not the transpiler.
- `Duration: 1.500s` — `fmt_sprintf` format specifier parsing was fixed (see earlier session)
- `Greet: Hello, world!` — string concatenation with proper borrowing works correctly
