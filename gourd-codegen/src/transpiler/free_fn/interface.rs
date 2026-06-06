//! Interface transpilation.
//!
//! Converts Go interface declarations to Rust trait declarations.

use super::super::parsing::{GoInterface};
use super::super::types::map_go_types;
use super::util::to_snake_case;
use proc_macro2::TokenStream;
use quote::quote;

/// Top-level: parse and transpile a Go interface declaration to Rust.
pub fn go_to_rust_interface(input: TokenStream) -> TokenStream {
    match syn::parse2::<GoInterface>(input) {
        Ok(go_interface) => {
            // Preserve Go interface name (camelCase stays camelCase)
            let trait_name = &go_interface.ident;

            // Transpile each interface method to a trait method
            let methods: Vec<_> = go_interface.methods.iter().map(|method| {
                let method_name_str = to_snake_case(&method.name.to_string());
                let method_name = syn::Ident::new(&method_name_str, method.name.span());

                // Parse method parameters
                let params: Vec<_> = method.inputs.args.iter().map(|param| {
                    let id = &param.id;
                    match (&param.ty, &param.slice_elem) {
                        (None, None) => quote! { #id },
                        (_, Some(slice_inner)) => {
                            let mapped = map_go_types(slice_inner);
                            quote! { #id: &[ #mapped ] }
                        }
                        (Some(ty), None) => {
                            let mapped = map_go_types(ty);
                            quote! { #id: #mapped }
                        }
                    }
                }).collect();

                // Parse return type
                let output = method.output.as_ref().map(|output| {
                    if output.tys.is_empty() {
                        quote! {} // No return
                    } else {
                        let mapped: Vec<_> = output.tys.iter().map(|t| map_go_types(t)).collect();
                        match mapped.len() {
                            1 => {
                                let m = &mapped[0];
                                if output.is_slice {
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

                // Method signature: `fn method_name(&self, params...) -> output;`
                quote! { fn #method_name(&self, #(#params),*) #output }
            }).collect();

            quote! {
                trait #trait_name { #(#methods);* }
            }
        }
        Err(e) => e.to_compile_error(),
    }
}
