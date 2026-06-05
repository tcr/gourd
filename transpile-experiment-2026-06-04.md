# Gourd Transpile Experiment — 2026-06-04

Real-world test: 27 Go functions of increasing complexity, run through `gourd transpile`.

## Summary

| Metric | Value |
|--------|-------|
| Go functions tested | 27 |
| Functions that transpile | 15 |
| Coverage | **55%** |
| Repeat patterns | 3 |

## What works (15 functions)

| Function | Feature tested |
|----------|---------------|
| `goSum` | Simple typed params |
| `goShorthand` | Grouped params (`a, b, c int`) |
| `goShorthand2` | Trailing digits in names |
| `goIsEven` | Boolean return |
| `goStringLen` | `len(s)` builtin |
| `goSwitch` | Selector switch → `match` |
| `goNoSelectorSwitch` | No-selector switch → `if/else if` |
| `goDensityClass` | Type conversions (`f64`, `int`) |
| `goStringSimilarity` | String comparisons, nested `if` |
| `goSplitWords` | Stdlib `strings.Split` + `strings.Join` |
| `goDivmod` | Multi-return |
| `goMapWithDefaults` | Map access with existence check |
| `goRangeLoop` | `for i, v := range arr` with `arr[i]` |
| `goIndexAccess` | `arr[idx]` indexing |
| `goForRangeMap` | `for k, v := range map` |

## What fails (12 functions)

| Function | Error | Root cause |
|----------|-------|------------|
| `goLongestWord` | "expected curly braces" | `for` loop with `<` in `if` condition |
| `goAvgWordLength` | "expected curly braces" | `for` loop with `<` in `if` condition |
| `goMapOperations` | "expected keyword `_`" | Underscore in `for _, v := range m` |
| `goFindDuplicates` | "expected curly braces" | `for` loop with `<` in `if` condition |
| `goBatchReport` | "expected curly braces" | Variadic params + `for` loop |
| `goTextSummary` | "expected curly braces" | `strings.Fields` + `for` loop |
| `goDataProcessor` | "expected curly braces" | `for` loop with `<` |
| `goWordCount` | "expected curly braces" | `for _, w := range` (range loop, fails on braces) |
| `goBatchCombined` | "expected curly braces" | `for` loop with `<` in `if` |
| `goCStyleLoop` | "expected an expression" | C-style `for` without `range` |
| `goRangeValueOnly` | "expected keyword `_`" | Underscore as range identifier |

## Three repeating patterns

### Pattern A: "expected curly braces" — 8 functions

**Cause:** The scanner/parser breaks on brace-delimited groups containing `<` comparison operators, or on `for`/`while`/`if` bodies that contain conditionals with `<`.

**Affected functions:** longestWord, avgWordLength, findDuplicates, batchReport, textSummary, dataProcessor, wordCount, batchCombined

**Fix needed:** In the scanner (`gourd-codegen/src/scanner.rs`), the `subtree` function needs to handle `<` inside brace groups as an expression operator, not as a generic type boundary. The original debug notes (2026-06-04) had a similar fix where `syn::parse_quote!` was replaced with `quote!` and `prettyplease::unparse` was replaced with raw token serialization.

### Pattern B: "expected keyword `_`" — 2 functions

**Cause:** The parser does not handle `_` as a valid range loop identifier. In Go, `_` is a placeholder that should be accepted but silently discarded.

**Affected functions:** mapOperations, rangeValueOnly

**Fix needed:** In the range loop parser, detect `_` as a special case and skip it rather than trying to parse it as an `Ident`.

### Pattern C: "expected an expression" — 1 function

**Cause:** C-style `for` loops (`for i := 0; i < n; i++`) without a `range` keyword are not transpiled.

**Affected function:** cStyleLoop

**Fix needed:** Add a new parser mode for C-style for loops. This is listed on the ROADMAP as "still not implemented."

## Recommendations

Priority order for fixing (by number of affected functions):

1. **Pattern A** (8 functions) — Fix `<` handling in brace groups. This is the highest-impact fix.
2. **Pattern B** (2 functions) — Handle `_` as range loop placeholder. Simple fix.
3. **Pattern C** (1 function) — Add C-style for loop support. More involved.

If all three patterns are fixed, coverage would jump from 55% to **96%**.
