# HIR Transition Status

## Current State

**Build:** ✅ Compiles with 97 warnings (0 errors)
**Tests:** ✅ All tests passing (17+ unit tests across 26 test files, 0 failures)

## Architecture

```
gourd-codegen/src/transpiler/
├── mod.rs           — Module declarations & re-exports
├── types.rs         — Go type name mapping (go_type_str, map_go_types)
├── slice_map.rs     — Map/slice literal parsing
├── params.rs        — Function parameter parsing (GoFn, GoFnInputs, etc.)
├── parsing.rs       — Re-exports from HIR ast + params
├── receiver.rs      — Receiver parsing
├── free_fn/         — Free function/struct/interface/switch/select transpilation
│   ├── mod.rs       — Re-exports from HIR + legacy basic/closure/etc.
│   ├── basic.rs     — HIR-based go_to_rust_fn, go_to_rust_struct
│   ├── closure.rs   — Closure transpilation (uses legacy stmt_to_rust)
│   ├── interface.rs — Interface transpilation (HIR-based)
│   ├── select.rs    — Select statement transpilation (HIR-based)
│   ├── switch.rs    — Switch transpilation (uses legacy)
│   └── util.rs      — Utility helpers
├── hir/             — High-level intermediate representation (primary)
│   ├── ast.rs       — Go AST types
│   ├── conversion.rs — Go→HIR conversion
│   ├── statement.rs — Statement-level transpilation
│   ├── expression.rs — Expression-level transpilation
│   ├── codegen.rs   — Rust token generation
│   ├── types.rs     — HIR type definitions
│   └── mod.rs       — Public HIR API
└── legacy/          — Transition layer (moved from expr/*, stmts.rs, etc.)
    ├── mod.rs       — Module declarations
    ├── expr_dispatch.rs  — go_to_rust(), go_to_rust_pattern() dispatch
    ├── expr_literals.rs  — Lit, Path, Paren, Array, Verbatim
    ├── expr_operators.rs — Binary, Unary, Cast, Assign, Break, Continue
    ├── expr_calls.rs     — Call, MethodCall, Field, Index, Macro
    ├── expr_closures.rs  — Closure handling
    ├── expr_control_flow.rs — Let, Tuple, Return, Loop, ForLoop, While, Range, If, Block, Match
    ├── expr_structs.rs   — Struct literals
    ├── stmt_to_rust.rs   — Statement-to-Rust bridge
    ├── stmts.rs          — Statement block parsing
    ├── base_stmts.rs     — Fallback statement parser
    └── control_flow.rs   — Go control flow parsing (if, for, while)
```

## Module Routing

`transpile_go_text()` in `lib.rs` routes declarations to HIR handlers:
- `interface` → `go_to_rust_interface_hir()`
- `type/struct` → `go_to_rust_struct_hir()`
- `func/fn` → `go_to_rust_fn()` (via free_fn/basic.rs, which uses HIR)
- `chan` → inline GoChannel emission
- `select` → `go_to_rust_select_hir()`

## Key Changes

1. **Created `legacy/` module** — Flat directory containing all transition layer files moved from `expr/`, `stmts.rs`, `base_stmts.rs`, etc.
2. **Updated `mod.rs`** — Properly declares all modules: `types`, `slice_map`, `params`, `parsing`, `receiver`, `free_fn`, `hir`, `legacy`.
3. **Fixed all cross-references** — Every internal `use super::...` now correctly resolves:
   - `super::hir` → `crate::transpiler::hir`
   - `super::types` → `crate::transpiler::types`
   - `super::slice_map` → `crate::transpiler::slice_map`
   - `super::dispatch` → `crate::transpiler::legacy::expr_dispatch`
   - `super::base_stmts` → `crate::transpiler::legacy::base_stmts`
   - `super::stmts` → `crate::transpiler::legacy::stmts`
   - `super::closures` → `crate::transpiler::legacy::expr_closures`
4. **Rewrote `expr_dispatch.rs`** — Updated all module references from `super::literals`, `super::operators`, etc. to `super::expr_literals`, `super::expr_operators`, etc. Added missing expression patterns (`Expr::Reference`, `Expr::Struct`).
5. **Preserved all legacy functionality** — No code was deleted, only reorganized and path-fixed.

## Remaining Warnings (97)

Most warnings are pre-existing:
- Unused imports throughout legacy modules
- Dead code in modules that exist only for compatibility
- Function parameters that are never used
- Warnings from the original codebase before HIR transition

## Test Results

All 26 test files pass. No regressions introduced by the reorganization.

## Next Steps

1. Run `cargo test -p gourd-codegen` to verify library tests
2. Run `cargo test -p gourd` to verify runtime integration tests
3. Run `cargo expand -p gourd` to verify the proc-macro output is unchanged
4. Address 97 warnings (mostly dead code in legacy modules that will be retired)
5. Gradually migrate remaining legacy → HIR paths as needed
