# Gourd Transpiler Improvements — Implementation Summary

## Features Implemented

### 1. `continue` Statement Support (dispatch.rs + operators.rs)
- Added `Expr::Continue` handler in the expression dispatcher
- Maps Go `continue [label]` to Rust `continue [label]`
- Minimal change, ~5 lines added

### 2. `strings` Standard Library Mapping
Added transpiler recognition and runtime for 16 string functions:

| Go Pattern | Rust Output | Runtime Function |
|------------|-------------|-----------------|
| `strings.Replace(s, old, new, n)` | `strings_replace(s, old, new, n)` | ✅ |
| `strings.ReplaceAll(s, old, new)` | `strings_replace_all(s, old, new)` | ✅ |
| `strings.HasPrefix(s, prefix)` | `has_prefix(s, prefix)` | ✅ |
| `strings.HasSuffix(s, suffix)` | `has_suffix(s, suffix)` | ✅ |
| `strings.Contains(s, substr)` | `contains_str(s, substr)` | ✅ |
| `strings.Split(s, sep)` | `split(s, sep)` | ✅ |
| `strings.Join(elems, sep)` | `join(elems, sep)` | ✅ |
| `strings.Index(s, substr)` | `index_str(s, substr)` | ✅ (already existed) |
| `strings.LastIndex(s, substr)` | `last_index_str(s, substr)` | ✅ |
| `strings.Trim(s)` | `trim(s)` | ✅ (already existed) |
| `strings.TrimLeft(s)` | `trim_left(s)` | ✅ (already existed) |
| `strings.TrimRight(s)` | `trim_right(s)` | ✅ (already existed) |
| `strings.ToUpper(s)` | `to_upper(s)` | ✅ (already existed) |
| `strings.ToLower(s)` | `to_lower(s)` | ✅ (already existed) |
| `strings.Repeat(s, n)` | `repeat(s, n)` | ✅ (already existed) |
| `strings.Fields(s)` | `fields(s)` | ✅ |

### 3. `os` Standard Library Mapping
Added transpiler recognition and runtime for 10 OS functions:

| Go Pattern | Rust Output | Runtime Function |
|------------|-------------|-----------------|
| `os.Open(path)` | `os_open(path)` | ✅ `std::fs::read(path)` |
| `os.ReadFile(path)` | `os_read_file(path)` | ✅ `std::fs::read(path)` |
| `os.WriteFile(path, data, perm)` | `os_write_file(path, data, perm)` | ✅ `std::fs::write(path, data)` |
| `os.Mkdir(path, perm)` | `os_mkdir(path, perm)` | ✅ `std::fs::create_dir(path)` |
| `os.MkdirAll(path, perm)` | `os_mkdir_all(path, perm)` | ✅ `std::fs::create_dir_all(path)` |
| `os.Remove(path)` | `os_remove(path)` | ✅ `std::fs::remove_file(path)` |
| `os.Chdir(path)` | `os_chdir(path)` | ✅ `std::env::set_current_dir(path)` |
| `os.Getenv(key)` | `os_getenv(key)` | ✅ `std::env::var(key)` |
| `os.Setenv(key, value)` | `os_setenv(key, value)` | ✅ `std::env::set_var(key, value)` |
| `os.Args` | `os_args()` | ✅ `std::env::args().collect()` |

## Files Modified

| File | Lines Changed | Description |
|------|--------------|-------------|
| `gourd-codegen/src/transpiler/expr/dispatch.rs` | +2 | Added `Expr::Continue` handler, changed `emit_todo` signature |
| `gourd-codegen/src/transpiler/expr/operators.rs` | +7 | Added `transpile_continue` function |
| `gourd-codegen/src/transpiler/expr/calls.rs` | +110 | Added `try_parse_strings_call`, `try_parse_os_call`, strings/os dispatch in `transpile_call` |
| `gourd/src/prelude.rs` | +100 | Added stdlib runtime functions for strings and os |

## Updated Real-World Coverage Estimate

| Code Category | Before | After | Notes |
|---------------|--------|-------|-------|
| Algorithmic/competitive code | ~60–70% | ~70–80% | +10% from strings support |
| CLI tools | ~30% | ~40–45% | +10% from os.Args, os env, os file I/O |
| Web server handlers | ~15% | ~20% | Still missing net/http |
| Data processing | ~20% | ~30% | +10% from strings, os file ops |
| Utility functions | ~25% | ~35% | +10% from strings, os, math |
| Infrastructure/DB | ~5% | ~8% | Still missing database/sql |

**Overall: ~15–20% → ~25–30% real-world Go compatibility**

## Still Missing (Major Gaps)

1. **Standard library mapping** — `net/http`, `encoding/json`, `database/sql`, `sync`, `time`, `io`, `reflect`, `rand`
2. **Generics** — `map[K]V` with custom types, `interface{}` → type parameters
3. **Variadic functions** — `func f(args ...int)` → `fn f(args: Vec<i32>)`
4. **Type switch** — `switch v := x.(type) { case int: ... }`
5. **Embedded types** — `type Server struct { net.Listener }`
6. **Channel operations** — `close(ch)`, `ch <- v` (statement-level)

## What Would Hit 50% Coverage

| Feature | Estimated Gain |
|---------|---------------|
| Standard library mapping (fmt, strings, os, encoding/json) | +10% |
| Generics | +8% |
| Variadic functions | +4% |
| Type switch | +2% |
| **Total** | **~50%** |
