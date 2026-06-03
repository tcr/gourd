//! Literal and path transpilation: `Lit`, `Path`, `Paren`, `Array`, `Verbatim`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{ExprArray, ExprLit, ExprParen, ExprPath};

use super::super::parsing::ElemParser;

pub fn transpile_lit(input: &ExprLit) -> TokenStream {
    let lit = &input.lit;
    match lit {
        syn::Lit::Str(s) => quote! { ::std::string::String::from(#s) },
        _                => quote! { #lit },
    }
}

/// Pattern variant: keep string literals as `&str` patterns.
pub fn transpile_lit_pattern(input: &ExprLit) -> TokenStream {
    let lit = &input.lit;
    match lit {
        syn::Lit::Str(s) => quote! { #s },  // &str pattern, not String::from
        _                => quote! { #lit },
    }
}

pub fn transpile_path(input: &ExprPath) -> TokenStream {
    let p = &input.path;
    match p.get_ident() {
        Some(ident) => match ident.to_string().as_str() {
            "nil"   => quote! { None },
            "true"  => quote! { true },
            "false" => quote! { false },
            _       => quote! { #p },
        },
        None => quote! { #p },
    }
}

pub fn transpile_paren(input: &ExprParen) -> TokenStream {
    let inner = super::dispatch::go_to_rust(&input.expr);
    quote! { ( #inner ) }
}

pub fn transpile_array(input: &ExprArray) -> TokenStream {
    let elems: Vec<_> = input.elems.iter().map(super::dispatch::go_to_rust).collect();
    if elems.is_empty() {
        // In Go slice literals like `[]int{ 1, 2, 3 }`, syn parses `[]`
        // as an empty array expression. The actual slice elements come
        // from the `Expr::Verbatim` handling below.
        quote! { vec![] }
    } else {
        quote! { [#(#elems),*] }
    }
}

/// Handle `Expr::Verbatim` tokens produced by syn when it can't fully
/// parse Go slice/map literals or anonymous functions.
pub fn transpile_verbatim(tokens: &proc_macro2::TokenStream) -> TokenStream {
    use proc_macro2::TokenTree;

    // Check for anonymous Go function: `func(params) ret { body }`
    // or `func(params) { body }` — no return type.
    if let Some(closure) = super::closures::parse_closure(tokens) {
        return super::closures::closure_to_rust(&closure);
    }

    // Check for slice/map literals
    for tt in tokens.clone() {
        if let TokenTree::Group(g) = tt
            && g.delimiter() == proc_macro2::Delimiter::Brace
        {
            let brace_content = g.stream();
            let parser: ElemParser = syn::parse2(brace_content).unwrap_or_default();
            let elems: Vec<_> = parser.elems.iter().map(|expr| super::dispatch::go_to_rust(expr)).collect();
            return quote! { vec![ #(#elems),* ] };
        }
    }

    // No brace group — emit raw tokens (simple literals)
    quote! { #tokens }
}
