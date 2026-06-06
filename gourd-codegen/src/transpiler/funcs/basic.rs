//! Basic receiver function transpilation.
//!
//! Converts parsed `ReceiverFn` AST into Rust `impl` block tokens.

use super::receiver::replace_receiver;
use super::super::expr::go_to_rust;
use super::super::receiver::{Receiver, ReceiverFn};
use super::super::types::map_go_types;
use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::token;

/// Transpile a receiver function to Rust: `impl Struct { fn method(...) { ... } }`
pub fn go_to_rust_receiver_fn(input: TokenStream) -> TokenStream {
    match syn::parse2::<ReceiverFn>(input) {
        Ok(parsed) => {
            let Receiver { name: recv_name, _ty: struct_ty, pointer } = parsed.recv;
            let fn_name = &parsed.ident;
            let generics = Punctuated::<syn::GenericParam, token::Comma>::new();

            // Receiver: `&mut self` for pointer, `&self` for value
            let self_arg = if pointer {
                quote! { &mut self }
            } else {
                quote! { &self }
            };

            // Map parameters (reuse GoFnInputs logic)
            let mut all_params = Vec::<TokenStream>::new();
            for param in &parsed.inputs.args {
                let id = &param.id;
                if let Some(_ty) = &param.ty {
                    match (&param.ty, &param.slice_elem) {
                        (Some(ty), None) => {
                            let mapped = map_go_types(ty);
                            all_params.push(quote! { #id: #mapped });
                        }
                        (Some(_ty), Some(slice_inner)) => {
                            let mapped = map_go_types(slice_inner);
                            all_params.push(quote! { #id: &[ #mapped ]});
                        }
                        _ => {}
                    }
                } else {
                    all_params.push(quote! { #id });
                }
            }

            // Map output
            let output = parsed.output.as_ref().map(|output| {
                if output.tys.is_empty() {
                    quote! {}
                } else {
                    let mapped: Vec<_> = output.tys.iter().map(|t| map_go_types(t)).collect();
                    match mapped.len() {
                        1 => {
                            let m = &mapped[0];
                            quote! { -> #m }
                        }
                        _ => quote! { -> ( #(#mapped),* ) },
                    }
                }
            }).unwrap_or_else(|| quote! {});

            // Transpile the body: For each statement, first rename the receiver
            // to "self" in the Go AST, then transpile to Rust via go_to_rust.
            let mut stmts: Vec<TokenStream> = Vec::new();
            for expr in &parsed.stmts {
                let renamed = replace_receiver(expr.clone(), &recv_name);
                let transpiled = go_to_rust(&renamed);
                stmts.push(transpiled);
            }

            let body: Box<syn::ExprBlock> = syn::parse_quote!({ #(#stmts);* });

            // Build the parameter list: if there are additional params, emit
            // `self_arg, ...params`. Otherwise just `self_arg` (no trailing
            // comma — `fn get(&self,)` is not valid Rust).
            let param_list = if all_params.is_empty() {
                quote! { #self_arg }
            } else {
                quote! { #self_arg, #(#all_params),* }
            };
            quote! {
                impl #generics #struct_ty {
                    fn #fn_name (#param_list) #output #body
                }
            }
        }
        Err(e) => {
            e.to_compile_error()
        }
    }
}
