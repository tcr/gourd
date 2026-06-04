//! Basic function and struct transpilation.
//!
//! Converts Go function declarations (`fn name() { ... }`) and struct
//! declarations (`struct Name { field type }`) into Rust.

use super::super::parsing::{GoFn, GoStruct};
use super::super::types::map_go_types;
use proc_macro2::TokenStream;
use quote::quote;

/// Top-level: parse and transpile a Go function declaration to Rust.
pub fn go_to_rust_fn(input: TokenStream) -> TokenStream {
    match syn::parse2::<GoFn>(input) {
        Ok(go_fn) => {
                // Preserve Go function name (camelCase stays camelCase)
            let fn_name = &go_fn.ident;
            let generics = &go_fn.generics;

            let output = go_fn.output.as_ref().map(|output| {
                if output.tys.is_empty() {
                    quote! {}
                } else {
                    let mapped: Vec<_> = output.tys.iter().map(map_go_types).collect();
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
                stmts.push(super::super::parsing::go_stmt_to_rust(stm));
            }
            let body: Box<syn::ExprBlock> = syn::parse_quote!({ #(#stmts);* });

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
