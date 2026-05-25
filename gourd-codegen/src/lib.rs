use proc_macro::TokenStream;
use syn::{Expr, parse_macro_input};

mod transpiler;

#[proc_macro]
pub fn go(input: TokenStream) -> TokenStream {
    let expr = parse_macro_input!(input as Expr);
    transpiler::go_to_rust(&expr).into()
}
