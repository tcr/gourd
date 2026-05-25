use proc_macro::TokenStream;
use syn::{Expr, parse_macro_input};

mod transpiler;

/// Re-export the expression macro so `use gourd_codegen::go_expr!` works.
/// This is NOT a proc-macro — it is just the existing function
/// exported at module scope so consumers can alias it whatever they want.
#[proc_macro]
pub fn go_expr(input: TokenStream) -> TokenStream {
    let expr = parse_macro_input!(input as Expr);
    transpiler::go_to_rust(&expr).into()
}

/// Top-level macro for Go function declarations.
///
/// Parses Go function syntax: `go! { func foo(a, b int) string { body } }`
/// → Rust `fn foo(a: i32, b: i32) -> String { body }`
///
/// Handles Go parameter shorthand `(a, b T)` → `(a: T, b: T)`.
#[proc_macro]
pub fn go(input: TokenStream) -> TokenStream {
    transpiler::go_to_rust_fn(input.into()).into()
}
