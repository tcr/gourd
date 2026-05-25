// ── Go feature coverage for gourd ──────────────────────────────────
// This file documents every Go feature the `go!` macro currently
// supports (covered by tests/coverage.rs and embed_tests.rs)
// and every feature that still triggers compile_error! at macro
// expansion time.
//
// ## Supported features (try them: coverage.rs)
//
// LITERALS:
//   Integer literals:   42, 0xff, 0o77, 1e3
//   Float literals:     3.14
//   String literals:    "hello"
//   Boolean literals:   true  /  false
//
// PATHS (constants):
//   nil     → Rust: None
//   true    → Rust: true
//   false   → Rust: false
//
// EXPRESSION BUILDERS:
//   Binary:  +  -  *  /  %  &&  ||  ^  &  |  <<  >>  ==  !=  >  >=  <  <=
//   Unary:   -  (negation)  !  (not)  *  (deref)
//   Paren:   (x)
//
// CALLS:
//   len(s) / cap(s) → s.len()
//   Ordinary calls:  String::from("hello")  →  calls transpile recursively
//
// SHORT DECLARATIONS:
//   x := 42  →  let x = 42
//
// ## Features triggering compile_error! (TODO)
//
// CONTROL FLOW:
//   if/else, if-init, for/init, for-range, switch, select,
//   defer, panic/recover, return, break, continue, goto
//
// DATA STRUCTURES:
//   struct{}, map{}, arrays, slices, channels
//
// TYPE SYSTEM:
//   interface{}, type assertions, type switches, type conversions
//
// CONCURRENCY:
//   go, <-, channel make, sync primitives
//
// ## How to evaluate coverage
//
//   cargo test --test coverage   ← currently passing tests
//   cargo test --test embed_tests  ← original arithmetic subset
//   cargo test --test unimplemented  ← documentation only
//
// To add a new feature:
//   1. Write a test case in coverage.rs for the Go construct
//   2. Add the matching Expr::Variant handler in transpiler.rs
//   3. Run `cargo test` — the test passes when the feature works
//
// Total Go features: ~40
// Supported: ~25 (expression-level, passed by coverage.rs)
// TODO: ~15 (control flow + advanced language constructs)
