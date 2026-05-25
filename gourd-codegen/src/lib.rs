use proc_macro::TokenStream;
use syn::{Expr, parse_macro_input};

mod transpiler;

/// Procedural macro that embeds Go code and transpiles it to Rust at
/// macro-expansion time (i.e. your crate's compile time).
///
/// ```ignore
/// use gourd_codegen::go;
///
/// let r = go! { 10 + 20 };
/// assert_eq!(r, 30i32);
/// ```
#[proc_macro]
pub fn go(input: TokenStream) -> TokenStream {
    let expr = parse_macro_input!(input as Expr);
    transpiler::go_to_rust(&expr).into()
}
