//! Select statement transpilation.
//!
//! Converts Go `select { ... }` to Rust `GoSelect::new().run()`.

use proc_macro2::TokenStream;
use quote::quote;

/// Transpile a select statement to Rust.
///
/// `select { case ... default: ... }` → `GoSelect::new().run()`
pub fn go_to_rust_select(input: TokenStream) -> TokenStream {
    // Extract the select block body
    let trees: Vec<proc_macro2::TokenTree> = input.into_iter().collect();
    if trees.len() < 2 {
        return quote! { { compile_error!("TODO: select statement") } };
    }

    // Find the brace group containing the select cases
    for tree in trees.iter().skip(1) {
        if let proc_macro2::TokenTree::Group(g) = tree {
            if g.delimiter() == proc_macro2::Delimiter::Brace {
                // Extract the body
                let body: TokenStream = g.stream();
                return quote! {
                    GoSelect::new().run(|| { #body })
                };
            }
        }
    }

    // Fallback
    quote! { { compile_error!("TODO: select statement") } }
}
