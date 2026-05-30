//! Free function and struct transpilation.
//!
//! Converts Go function declarations (`fn name() { ... }`) and struct
//! declarations (`struct Name { field type }`) into Rust.

use super::expr::go_to_rust;
use super::parsing::{go_stmt_to_rust, GoFn, GoStruct, Switch};
use super::types::map_go_types;
use proc_macro2::TokenStream;
use quote::quote;

/// Top-level: parse and transpile a Go function declaration to Rust.
pub fn go_to_rust_fn(input: TokenStream) -> TokenStream {
    match syn::parse2::<GoFn>(input) {
        Ok(go_fn) => {
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
                            quote! { -> #m }
                        }
                        _ => quote! { -> ( #(#mapped),* ) },
                    }
                }
            }).unwrap_or_else(|| quote! {});

            let mut all_params = Vec::<TokenStream>::new();
            for param in &go_fn.inputs.args {
                let id = &param.id;
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
                        all_params.push(quote! { #id: #mapped });
                    }
                }
            }

            let mut stmts = Vec::new();
            for stm in &go_fn.block.stmts {
                stmts.push(super::parsing::go_stmt_to_rust(stm));
            }
            let body: Box<syn::ExprBlock> = syn::parse_quote!({ #(#stmts);* });

            quote! {
                fn #fn_name #generics ( #(#all_params),* ) #output #body
            }
        }
        Err(e) => e.to_compile_error(),
    }
}

/// Top-level: parse and transpile a Go switch statement to Rust.
pub fn go_to_rust_switch(input: TokenStream) -> TokenStream {
    match syn::parse2::<Switch>(input) {
        Ok(switch) => transpile_switch(&switch),
        Err(e) => e.to_compile_error(),
    }
}

pub(crate) fn transpile_switch(switch: &Switch) -> TokenStream {
    // Build match arms from case expressions
    let mut arms = Vec::new();

    for case in &switch.cases {
        if case.exprs.is_empty() {
            // Empty exprs means this is a default-like case
            // but we handle default separately
            continue;
        }

        // Each case expression becomes a pattern in Rust match
        // Multiple expressions in one case are comma-separated patterns
        let pattern: Vec<_> = case.exprs.iter().map(|e| go_to_rust(e)).collect();
        let body: Vec<_> = case.stmts.iter().map(|s| go_stmt_to_rust(s)).collect();

        // Single or multi-expression case
        arms.push(quote! { #(#pattern),* => { #(#body);* } });
    }

    // Handle default case with `_` pattern
    if !switch.default_stmts.is_empty() {
        let default_body: Vec<_> = switch.default_stmts.iter()
            .map(|s| go_stmt_to_rust(s))
            .collect();
        arms.push(quote! { _ => { #(#default_body);* } });
    }

    // Build selector
    let selector = switch.selector.as_ref()
        .map(|s| go_to_rust(s))
        .unwrap_or_else(|| quote! { () });

    quote! { match #selector { #(#arms),* } }
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
