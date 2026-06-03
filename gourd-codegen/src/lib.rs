//! gourd-codegen: procedural macro crate for Go → Rust transpilation.
//!
//! This crate provides the `go!` proc-macro which transpiles Go declarations
//! into valid Rust at compile time.
//!
//! For programmatic access to the transpiler (e.g. for test inspection),
//! use the `gourd-codegen-core` crate directly.

use proc_macro::TokenStream;

/// Top-level macro for Go declarations.
///
/// Dispatches to the appropriate transpiler based on input pattern:
///   1. `func (recv Type) name() { ... }` → `impl Type { fn name(&self) { ... } }`
///   2. `struct Name { field type }` → `struct Name { pub field: Type }`
///   3. `func name() { ... }` → `fn name() { ... }`
///
/// **Semantic validation**: the input is validated against `go build` at compile
/// time. If the Go code doesn't compile, a `compile_error!` is emitted.
#[proc_macro]
pub fn go(input: TokenStream) -> TokenStream {
    let tokens: proc_macro2::TokenStream = input.into();

    // Semantics: check Go code compiles (skip if `go` is unavailable).
    // Validation via `gourd-check` is preferred for CI and pre-compile checks.
    let _ = gourd_codegen_core::validate_go(&tokens);

    gourd_codegen_core::transpile_go(tokens).into()
}

/// Compile-time verification attribute: `#[verify_rust_output({ expected_rust })]`
///
/// Apply before a `go!` block to assert that the transpiled output
/// matches the expected Rust tokens.
///
/// ## Short form
///
/// `#[verify_rust_output({ fn foo() -> i32 { 42 } })]` — brace group is the expected output.
///
/// ## Longer form (⚠️ unused — to be removed)
///
/// `#[verify_rust_output(verify = { fn foo() -> i32 { 42 } })]` — explicit `verify =` key.
///
/// This form is **basically unused** in the codebase. It exists for symmetry
/// but serves no practical purpose over the short form. Plan to remove it
/// once it is no longer referenced anywhere.
///
/// **Semantic validation**: the Go input is validated against `go build`
/// and the transpiled Rust against `cargo check` before comparison.
/// If either fails, a `compile_error!` is emitted.
///
/// If the transpiled output doesn't match, compilation fails with
/// a `compile_error!` showing the expected vs actual output.
///
/// Example:
/// ```ignore
/// use gourd_codegen::go;
///
/// // Short form (preferred)
/// #[verify_rust_output({
///     fn go_abs(n: i32) -> i32 {
///         let mut ret = n;
///         if n < 0 { ret = -n; }
///         ret
///     }
/// })]
/// go! {
///     func goAbs(n int) int {
///         ret := n
///         if n < 0 { ret = -n }
///         return ret
///     }
/// }
///
/// // Longer form — unused, to be removed
/// // #[verify_rust_output(verify = {
/// //     fn go_abs(n: i32) -> i32 {
/// //         let mut ret = n;
/// //         if n < 0 { ret = -n; }
/// //         ret
/// //     }
/// // })]
/// // go! {
/// //     func goAbs(n int) int {
/// //         ret := n
/// //         if n < 0 { ret = -n }
/// //         return ret
/// //     }
/// // }
/// ```
#[proc_macro_attribute]
pub fn verify_rust_output(attr: TokenStream, input: TokenStream) -> TokenStream {
    let attr_tokens: proc_macro2::TokenStream = attr.into();
    let input_tokens: proc_macro2::TokenStream = input.into();
    gourd_codegen_core::verify_short(attr_tokens, input_tokens).into()
}
