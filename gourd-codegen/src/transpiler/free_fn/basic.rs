//! Basic function and struct transpilation.
//!
//! Converts Go function declarations (`fn name() { ... }`) and struct
//! declarations (`struct Name { field type }`) into Rust.
//!
//! HIR-based transpilation is available via `go_to_rust_fn_hir()`.

use crate::transpiler::parsing::{GoFn, GoStruct};
use super::super::types::map_go_types;
use proc_macro2::TokenStream;
use quote::quote;

/// Top-level: parse and transpile a Go function declaration to Rust.
/// Preprocess a token stream to convert Go slice range syntax `[start:end]`
/// to Rust slice range syntax `[start..end]`.
///
/// In CLI context, `[1:3]` is tokenized as a single `Group(Bracket)` token
/// containing the colon. We preprocess the group's content to replace `:`
/// with `..` so that `syn` can parse it as a Rust range expression.
fn preprocess_slice_ranges(ts: TokenStream) -> TokenStream {
    use proc_macro2::{TokenTree, Group, Delimiter, Punct, Spacing};

    /// Preprocess only **bracket** groups to convert colons to `..`.
    /// Brace groups (function bodies, struct bodies) are left untouched
    /// to avoid corrupting switch/case labels like `case 2:`.
    fn preprocess_bracket_groups(group: &Group) -> TokenStream {
        let tts: Vec<TokenTree> = group.stream().into_iter().collect();
        let mut result = Vec::new();

        for tt in tts {
            match tt {
                // Replace colons with `..` inside bracket groups only
                TokenTree::Punct(p) if p.as_char() == ':' => {
                    result.push(TokenTree::Punct(Punct::new('.', Spacing::Joint)));
                    result.push(TokenTree::Punct(Punct::new('.', Spacing::Alone)));
                }
                // Recursively preprocess nested groups — only if parent is bracket
                TokenTree::Group(inner_g)
                    if group.delimiter() == Delimiter::Bracket =>
                {
                    let inner_ts = preprocess_bracket_groups(&inner_g);
                    result.push(TokenTree::Group(Group::new(inner_g.delimiter(), inner_ts)));
                }
                _ => {
                    result.push(tt);
                }
            }
        }
        result.into_iter().collect()
    }

    ts.into_iter().map(|tt| {
        match tt {
            // Only preprocess bracket groups; leave brace groups alone
            TokenTree::Group(g)
                if g.delimiter() == Delimiter::Bracket =>
            {
                let inner = preprocess_bracket_groups(&g);
                TokenTree::Group(Group::new(Delimiter::Bracket, inner))
            }
            _ => tt,
        }
    }).collect()
}

pub fn go_to_rust_fn(input: TokenStream) -> TokenStream {
    let input = preprocess_slice_ranges(input);
    match syn::parse2::<GoFn>(input) {
        Ok(go_fn) => {
                // Preserve Go function name (camelCase stays camelCase)
            let fn_name = &go_fn.ident;
            let generics = &go_fn.generics;

            let output = go_fn.output.as_ref().map(|output| {
                if output.tys.is_empty() {
                    quote! {}
                } else {
                    let mapped: Vec<_> = output.tys.iter().map(|t| map_go_types(t)).collect();
                    match mapped.len() {
                        1 => {
                            let m = &mapped[0];
                            if output.is_slice {
                                // Use the stored element type for slices
                                if let Some(elem) = &output.elem_type {
                                    let mapped_elem = map_go_types(elem);
                                    quote! { -> Vec< #mapped_elem > }
                                } else {
                                    quote! { -> Vec< #m > }
                                }
                            } else {
                                quote! { -> #m }
                            }
                        }
                        _ => quote! { -> ( #(#mapped),* ) },
                    }
                }
            }).unwrap_or_else(|| quote! {});

            let mut all_params = Vec::<TokenStream>::new();
            for param in &go_fn.inputs.args {
                let id = &param.id;
                let variadic = param.variadic;
                match (&param.ty, &param.slice_elem) {
                    (None, None) => {
                        all_params.push(quote! { #id });
                    }
                    (_, Some(slice_inner)) => {
                        let mapped = map_go_types(slice_inner);
                        all_params.push(quote! { #id: &[ #mapped ]});
                    }
                    (Some(ty), None) => {
                        let mapped = map_go_types(ty);
                        if variadic {
                            // Variadic: `nums ...int` → `nums: &[i32]`
                            all_params.push(quote! { #id: &[ #mapped ] });
                        } else {
                            all_params.push(quote! { #id: #mapped });
                        }
                    }
                }
            }

            let mut stmts = Vec::new();
            for stm in &go_fn.block.stmts {
                stmts.push(crate::transpiler::parsing::go_stmt_to_rust(stm));
            }

            // If function has a return type and body is not empty,
            // wrap the last statement with `return` so it becomes the function's return value.
            let body: proc_macro2::TokenStream = if go_fn.output.is_some() && !stmts.is_empty() {
                let last = stmts.pop().unwrap();
                // Check if the last statement already starts with `return` keyword
                let last_str = last.to_string();
                let already_returns = last_str.trim_start().starts_with("return ") || last_str.trim_start() == "return";
                // Also check if it's a local declaration (let ...) — don't wrap with return
                let is_local = last_str.trim_start().starts_with("let ");
                if already_returns || is_local {
                    // Last statement already has `return` — just use it as-is
                    if stmts.is_empty() {
                        quote! { { #last } }
                    } else {
                        let all_but_last = &stmts;
                        quote! { { #(#all_but_last);*; #last } }
                    }
                } else {
                    // Last statement needs `return` wrapper
                    if stmts.is_empty() {
                        quote! { { return #last } }
                    } else {
                        let all_but_last = &stmts;
                        quote! { { #(#all_but_last);*; return #last } }
                    }
                }
            } else {
                quote!({ #(#stmts);* })
            };

            let body_str = body.to_string();
            eprintln!("DEBUG: FINAL body for {} = [{}]", go_fn.ident, body_str);

            let result = quote! {
                fn #fn_name #generics ( #(#all_params),* ) #output #body
            };
            result
        }
        Err(e) => {
            e.to_compile_error()
        }
    }
}

/// Top-level: parse and transpile a Go struct declaration to Rust.
pub fn go_to_rust_struct(input: TokenStream) -> TokenStream {
    match syn::parse2::<GoStruct>(input) {
        Ok(go_struct) => {
            let name = &go_struct.ident;
            let fields = go_struct.fields.iter().map(|f| {
                let fname = &f.name;
                let ftty = map_go_types(&f.ty);
                quote! { pub #fname: #ftty }
            });
            quote! {
                struct #name {
                    #(#fields),*
                }
            }
        }
        Err(e) => e.to_compile_error(),
    }
}
