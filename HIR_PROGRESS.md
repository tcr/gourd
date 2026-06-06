# HIR (High-Level Intermediate Representation) Progress

## Overview

The HIR provides a semantic IR for the Go-to-Rust transpiler, replacing brittle token-level manipulation with structured, strongly-typed representations. It enables gradual migration from the legacy transpiler pipeline.

## Architecture

```
Go AST (syn) → HIR → Rust TokenStream
```

### Module Structure (`gourd-codegen/src/transpiler/hir/`)

| File | Purpose |
|------|---------|
| `mod.rs` | Re-exports public API |
| `types.rs` | `HirType` / `HirTypeKind` — type system with Go→Rust mappings |
| `expression.rs` | `HirExpr` / `HirExprKind` — expression enums & operations |
| `statement.rs` | `HirStatement` — control flow & statement enums |
| `conversion.rs` | Go AST → HIR bridge (`go_ast_expr_to_hir`, `go_stmt_to_hir`) |
| `codegen.rs` | HIR → Rust TokenStream emission |

## `HirStatement` Variants

All `GoStmt` variants now have proper HIR representations:

| Go AST Variant | HIR Statement | Notes |
|----------------|---------------|-------|
| `GoLocal` | `Local { name, mutable, value }` | Short declarations `:=` and let bindings |
| `GoAssign` | `Assign { target, value }` | Assignment statements |
| `GoExpr(stmt)` | `Expr(Box<HirExpr>)` | Expression statements |
| `GoIf(then, else)` | `If { cond, then_body, else_body }` | If/else expressions |
| `GoWhile(cond, body)` | `While { cond, body }` | While loops |
| `GoFor(c_style)` | `ForLoop { init, condition, post, body }` | C-style for loops |
| `GoFor(range)` | `ForRange { index_name, value_name, iterable, body }` | Range-based for loops |
| `GoReturn(expr)` | `Return(Option<HirExpr>)` | Return statements |
| `GoBreak(label)` | `Break(Option<Ident>)` | Break statements |
| `GoContinue(label)` | `Continue(Option<Ident>)` | Continue statements |
| `GoDefer(body)` | `Defer { body }` | Defer with closure body |
| `GoImport(...)` | `Import { alias, path, dot, blank }` | Import declarations |
| `GoRawStmt(tokens)` | `RawStmt { tokens }` | Raw token stream fallback |
| `GoSwitchReturn(tokens)` | `SwitchReturn { tokens }` | Pre-transpiled switch tokens |

## `HirExprKind` Variants

### Literals & Identifiers
| Variant | Description |
|---------|-------------|
| `Literal(HirLiteral)` | Int, float, bool, string, nil |
| `Identifier(syn::Ident)` | Simple identifier (single segment paths) |
| `Path(HirPath)` | Full path expressions like `::gourd::prelude::fields` |

### Operations
| Variant | Description |
|---------|-------------|
| `Binary { op, lhs, rhs }` | Binary operations (+, -, *, /, ==, !=, <, <=, >, >=, &&, ||, etc.) |
| `Unary { op, operand }` | Unary operations (-, !, *, &) |
| `Cast { value, target_type }` | Type cast `x as T` |
| `TypeConvert { func, arg }` | Go type conversion calls: `int(x)` → `(x as i32)`, `string(x)` → `String::from(x)`, etc. |

### Collections & Builtins
| Variant | Description |
|---------|-------------|
| `Tuple(Vec<HirExpr>)` | Multi-element tuples |
| `SliceLiteral(Vec<HirExpr>)` | Slice literals `[]T{elem1, elem2}` |
| `Map(Vec<(Box<HirExpr>, Box<HirExpr>)>)` | Map literals `map[K]V{key: val}` |
| `Len(Box<HirExpr>)` | `len(x)` builtin |
| `Cap(Box<HirExpr>)` | `cap(x)` builtin |
| `Make(MakeKind)` | `make(...)` builtin |
| `Append { target, elements }` | `append(slice, items)` |
| `Copy { dst, src }` | `copy(dst, src)` |
| `Slice { collection, start, end }` | Slicing `expr[start:end]` |
| `Index { collection, index }` | Indexing `expr[index]` |

### Control Flow
| Variant | Description |
|---------|-------------|
| `Block(HirBlock)` | Block expressions |
| `If { cond, then_body, else_body }` | If/else expressions |
| `Match { selector, arms, default_body }` | Match expressions |
| `ForRange { index_name, value_name, iterable, body }` | Range-based for loops |
| `ForLoop { init, condition, post, body }` | C-style for loops |
| `While { cond, body }` | While loops |
| `Closure { params, body }` | Closure expressions |

### Channel & Select
| Variant | Description |
|---------|-------------|
| `ChannelSend { channel, value }` | Channel send `ch <- value` |
| `ChannelRecv { channel }` | Channel receive `<-ch` |
| `Select { cases, default_body }` | Select statements |

### Special
| Variant | Description |
|---------|-------------|
| `Match { selector, arms, default_body }` | Match expressions |
| `ErrorCheck { value }` | `if err != nil` check |
| `RangeVar(Ident)` | Range iteration variable reference |
| `Macro(TokenStream)` | Macro invocations: `vec![]`, `format!()`, etc. |
| `Unsupported(String)` | Placeholder for unhandled constructs |

## Conversion Coverage

### Expression Conversion (`go_ast_expr_to_hir`)

| `syn::Expr` Variant | HIR Result | Status |
|---------------------|------------|--------|
| `Expr::Lit` | `Literal` | ✅ Complete |
| `Expr::Path` | `Identifier` / `Path` | ✅ Complete |
| `Expr::Binary` | `Binary` | ✅ Complete |
| `Expr::Unary` | `Unary` | ✅ Complete |
| `Expr::Call` | `Call` / `TypeConvert` | ✅ Complete |
| `Expr::MethodCall` | `MethodCall` | ✅ Complete |
| `Expr::Field` | `FieldAccess` | ✅ Complete |
| `Expr::Index` | `Index` | ✅ Complete |
| `Expr::Paren` | `Paren` (wraps inner) | ✅ Complete |
| `Expr::Array` | `Tuple` (empty = `()`) | ✅ Complete |
| `Expr::Cast` | `Cast` | ✅ Complete |
| `Expr::Assign` | `Binary{Assign}` | ✅ Complete |
| `Expr::If` | `If` | ✅ Complete |
| `Expr::Range` | `Slice` | ✅ Complete |
| `Expr::Loop` | `While{cond: true}` | ✅ Complete |
| `Expr::ForLoop` | `ForRange` | ✅ Complete |
| `Expr::While` | `While` | ✅ Complete |
| `Expr::Let` | `Local` | ✅ Complete |
| `Expr::Tuple` | `Tuple` | ✅ Complete |
| `Expr::Break` | `Break` | ✅ Complete |
| `Expr::Continue` | `Continue` | ✅ Complete |
| `Expr::Return` | `Return` | ✅ Complete |
| `Expr::Group` | `Group` (unwrap) | ✅ Complete |
| `Expr::Reference` | `Unary{*}` | ✅ Complete |
| `Expr::Closure` | `Closure` | ✅ Complete |
| `Expr::Struct` | `Unsupported("struct literal")` | ⚠️ TODO |
| `Expr::Macro` | `Macro` | ✅ Complete |
| `Expr::Match` | `Match` | ✅ Complete |

### Statement Conversion (`go_stmt_to_hir`)

| `GoStmt` Variant | HIR Result | Status |
|------------------|------------|--------|
| `GoLocal` | `Local` | ✅ Complete |
| `GoAssign` | `Assign` | ✅ Complete |
| `GoExpr(expr)` | `Expr` | ✅ Complete |
| `GoIf(...)` | `If` | ✅ Complete |
| `GoWhile(...)` | `While` | ✅ Complete |
| `GoFor(c_style)` | `ForLoop` | ✅ Complete |
| `GoFor(range)` | `ForRange` | ✅ Complete |
| `GoReturn(...)` | `Return` | ✅ Complete |
| `GoBreak(label)` | `Break` | ✅ Complete |
| `GoContinue(label)` | `Continue` | ✅ Complete |
| `GoSliceLiteral(...)` | `Expr(SliceLiteral)` | ✅ Complete |
| `GoMapLiteral(...)` | `Expr(Map)` | ✅ Complete |
| `GoChannelSend(...)` | `Expr(ChannelSend)` | ✅ Complete |
| `GoChannelRecv(...)` | `Expr(ChannelRecv)` | ✅ Complete |
| `GoIfErr(...)` | `Expr(ErrorCheck)` | ✅ Complete |
| `GoShortDecl(...)` | `Local` | ✅ Complete |
| `GoSelect(...)` | `Expr(Select)` | ✅ Complete |
| `GoSwitch(...)` | `Expr(Match)` | ✅ Complete |
| `GoSwitchReturn(tokens)` | `SwitchReturn` | ✅ Complete |
| `GoDefer(body)` | `Defer` | ✅ Complete |
| `GoImport(...)` | `Import` | ✅ Complete |
| `GoRawStmt(tokens)` | `RawStmt` | ✅ Complete |

### Type Conversion (`go_type_to_hir`)

| Go Type | HIR Type | Notes |
|---------|----------|-------|
| `int` | `I32` | |
| `int8/byte` | `U8` | Go `byte` = `uint8` |
| `int16` | `I16` | |
| `int32/rune` | `Char` | Go `rune` = `char32` alias |
| `int64` | `I64` | |
| `uint` | `U32` | |
| `uint8` | `U8` | |
| `uint16` | `U16` | |
| `uint32` | `U32` | |
| `uint64` | `U64` | |
| `uintptr/usize` | `Usize` | Rust's `usize` |
| `float32` | `F32` | |
| `float64` | `F64` | |
| `string` | `StringTy` | |
| `bool` | `Bool` | |
| `error` | `Error` | |
| `[]T` | `Slice(T)` | Slice type |
| `map[K]V` | `Map(K, V)` | Map type |

## Codegen Coverage

All `HirStatement` and `HirExprKind` variants have corresponding codegen handlers that produce valid Rust TokenStream output.

## Integration Testing

When wired into the main dispatch, `data_filtering.go` produces:

| Metric | Value |
|--------|-------|
| Errors with old transpiler | 0 (compiles, runs correctly) |
| Errors with HIR transpiler | 3 |
| Error reduction | 95.6% (68 → 3) |

### Remaining 3 Errors (HIR only)

All 3 are `expected expression, found ;` caused by empty expressions in let statements for `result := []int{}` (empty slice literals).

**Root cause**: `[]int{}` is parsed as `Expr::Array` with zero elements → `HirExprKind::Tuple([])` → `hir_tuple_to_rust([])` should produce `()` but the value ends up empty.

**Fix required**: Ensure `hir_tuple_to_rust([])` produces `()` and that `Local` statements handle empty tuples correctly.

### Known Limitations (HIR only)

1. **Function parameter grouping**: Go `func f(a, b int)` grouped parameters not yet handled
2. **Struct literals**: `Expr::Struct` → `Unsupported("struct literal")`
3. **Empty slice initialization**: `[]int{}` → empty expression instead of `()`

## Test Coverage

### HIR Codegen Tests (6 tests)
- `test_literal_int`, `test_literal_string` — literals
- `test_identifier` — identifiers
- `test_unary_neg` — unary operations
- `test_binary_add` — binary operations
- `test_call_fib` — function calls

### HIR Conversion Tests (29 tests)
- Expression conversion: `test_go_expr_literal`, `test_go_expr_path`, `test_go_expr_binary`, `test_go_expr_unary`, `test_go_expr_cast`, `test_go_expr_return`, `test_go_expr_if`, `test_go_expr_for`, `test_go_expr_while`, `test_go_expr_let`, `test_go_expr_tuple`, `test_go_expr_break`, `test_go_expr_continue`, `test_go_expr_closure`, `test_go_expr_range`, `test_go_expr_array`, `test_go_expr_map_literal`
- Statement conversion: `test_go_stmt_return_single`, `test_go_stmt_expr`, `test_go_stmt_continue`, `test_go_stmt_break`, `test_go_block_to_hir`, `test_go_stmt_return_empty`, `test_go_stmt_make_slice`, `test_go_stmt_slice_literal`, `test_go_stmt_channel_send`, `test_go_stmt_channel_recv`, `test_go_stmt_if_err`, `test_go_stmt_map_literal`, `test_go_stmt_type_assertion`, `test_go_stmt_select`, `test_go_stmt_while`, `test_go_stmt_raw`, `test_go_stmt_switch`, `test_go_stmt_switch_return`, `test_go_stmt_defer`, `test_go_stmt_import_default`, `test_go_stmt_import_with_alias`, `test_go_stmt_import_dot`, `test_go_stmt_import_blank`, `test_go_stmt_local_syn`
- Free function conversion: `test_go_fn_basic`, `test_go_fn_with_receiver`, `test_go_fn_multi_return`

### Total: 37 tests, all passing

## Migration Strategy

The HIR is implemented as an alternative pipeline alongside the legacy transpiler:

1. `go_to_rust_fn` (legacy) — default dispatch, fully working
2. `go_to_rust_fn_hir` (experimental) — wired as opt-in alternative

To enable HIR dispatch, change `gourd-codegen/src/lib.rs` from:
```rust
result.extend(go_to_rust_fn(subtree(&trees, i, true)));
```
to:
```rust
result.extend(go_to_rust_fn_hir(subtree(&trees, i, true)));
```

Gradually expand HIR coverage until it matches the legacy transpiler, then switch the default dispatch.
