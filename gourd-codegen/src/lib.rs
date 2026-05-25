use proc_macro::TokenStream;
use syn::Expr;

fn parse_go_tokens(input: TokenStream) -> syn::Result<Expr> {
    let conn = proc_macro2::TokenStream::from(input);
    syn::parse2::<Expr>(conn)
}

mod transpiler;

#[proc_macro]
pub fn go(input: TokenStream) -> TokenStream {
    let parsed = match parse_go_tokens(input) {
        Ok(e) => e,
        Err(e) => return e.to_compile_error().into(),
    };
    transpiler::go_to_rust(&parsed).into()
}
