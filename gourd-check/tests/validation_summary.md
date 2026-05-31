# gourd-check Validation Summary

## Overall Results

```
Total blocks scanned:  31
Blocks with Go errors: 27
Blocks that pass:       4
Pass rate:             12.9%
```

## File-by-File Breakdown

| Test File | Valid | Total | Pass Rate |
|-----------|-------|-------|-----------|
| go_fn.rs | 4/21 | 21 | 19.0% |
| receiver_tests.rs | 0/5 | 5 | 0% |
| shorthand_query.rs | 0/2 | 2 | 0% |
| switch_extended.rs | 0/2 | 2 | 0% |
| switch_minimal.rs | 0/1 | 1 | 0% |

## The 4 Passing Blocks

| File | Line | Content | Why It Passes |
|------|------|---------|---------------|
| go_fn.rs | 9 | `func goAdd() int { return 42 }` | Pure Go syntax |
| go_fn.rs | 37 | `func goAbs(n int) int { ... }` | Pure Go syntax |
| go_fn.rs | 58 | `// go! { ... }` | Commented out (ignored) |
| go_fn.rs | 322 | `// go! { ... }` | Commented out (ignored) |

## The 27 Failing Blocks — Root Cause

All 27 failing blocks use **Rust syntax** inside `go!` blocks instead of Go syntax:

```
Rust syntax in go! blocks → What Go expects
─────────────────────────────────────────────
fn name(params) return    → func name(params) return
n: i32                    → n int
s: String                 → s string
return expr               → (keep as-is)
String::from("x")         → "x"
Vec<i32>                  → []int
m.get(2).unwrap()         → m[2]
a == b                    → a == b
len(a)                    → len(a)
[1,2,3]                   → []int{1,2,3}
```

### Error Categories

1. **`syntax error: non-declaration statement outside function body`** (22 blocks)
   - The transpiler outputs Rust syntax (`fn`, `->`, `i32`, `String::from`) 
   - Go parser can't handle Rust keywords/types

2. **`missing return` / `not used`** (1 block, line 21)
   - Block is valid Go syntax but the transpiler misses `return`
   - `func goSum(a int, b int) int { a + b }` needs `return a + b`

3. **`undefined: Foo`** (2 blocks, receiver_tests.rs)
   - Go struct type `Foo` referenced but not defined in the same file
   - Would need `struct Foo { x int }` defined first

## Key Insight

The `gourd-check` validator checks **raw source text** before macro expansion.
The test files were written with **Rust syntax** (because the tests verify
the Rust output). But the validator expects **Go syntax** (because `go!` blocks
should contain Go input).

This is expected behavior — it correctly identifies that the test files
don't use Go syntax inside `go!` blocks. The fix would be to rewrite the
test files to use Go syntax (which is what `gourd` is designed to accept).

## How to Fix

Convert all `go!` block content from Rust syntax to Go syntax:

```bash
# Before (Rust syntax — fails gourd-check):
go! {
    fn is_even(n: i32) -> bool {
        n % 2 == 0
    }
}

# After (Go syntax — passes gourd-check):
go! {
    func isEven(n int) bool {
        return n % 2 == 0
    }
}
```

## Usage

```bash
# Validate all test files
./target/debug/gourd-check gourd-codegen/tests/

# Validate a single file
./target/debug/gourd-check gourd-codegen/tests/go_fn.rs

# Verbose: show extracted blocks
./target/debug/gourd-check -v 2 gourd-codegen/tests/go_fn.rs

# Go-only validation
./target/debug/gourd-check -g gourd-codegen/tests/
```
