//! Free function and struct transpilation.
//!
//! Converts Go function declarations (`fn name() { ... }`) and struct
//! declarations (`struct Name { field type }`) into Rust.
//!
//! Go uses camelCase for exported names (e.g., `goAdd`). The transpiler
//! converts these to Rust snake_case (e.g., `go_add`).

use super::expr::{go_to_rust, go_to_rust_pattern};
use super::parsing::{go_stmt_to_rust, GoFn, GoStruct, Switch};
use super::types::map_go_types;
use proc_macro2::TokenStream;
use quote::quote;

/// Convert a Go name (camelCase) to Rust snake_case.
/// `goAdd` → `go_add`, `goShorthand2` → `go_shorthand_2`
/// Handles consecutive caps and trailing digits.
fn to_snake_case(name: &str) -> String {
    let mut result = String::with_capacity(name.len() + 4);
    let chars: Vec<char> = name.chars().collect();
    for (i, ch) in chars.iter().enumerate() {
        if ch.is_uppercase() {
            if i > 0 && !name[..i].ends_with('_') {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
        } else if ch.is_ascii_digit() && i > 0 && chars[i - 1].is_lowercase() {
            // Add underscore before digit if preceded by lowercase
            result.push('_');
            result.push(*ch);
        } else {
            result.push(*ch);
        }
    }
    result
}

/// Top-level: parse and transpile a Go function declaration to Rust.
pub fn go_to_rust_fn(input: TokenStream) -> TokenStream {
    match syn::parse2::<GoFn>(input) {
        Ok(go_fn) => {
            let fn_name_str = to_snake_case(&go_fn.ident.to_string());
            let fn_name = syn::Ident::new(&fn_name_str, go_fn.ident.span());
            let fn_name = &fn_name;
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

        // Case expressions become match patterns (string literals stay as &str)
        let pattern: Vec<_> = case.exprs.iter().map(|e| go_to_rust_pattern(e)).collect();
        let body: Vec<_> = case.stmts.iter().map(|s| go_stmt_to_rust(s)).collect();

        // Single or multi-expression case
        // Multi-expr: `case 1, 2, 3:` → `1 | 2 | 3 =>`
        arms.push(quote! { #(#pattern)|* => { #(#body);* } });
    }

    // Handle default case with `_` pattern
    if !switch.default_stmts.is_empty() {
        let default_body: Vec<_> = switch.default_stmts.iter()
            .map(|s| go_stmt_to_rust(s))
            .collect();
        arms.push(quote! { _ => { #(#default_body);* } });
    }

    // When there's no selector, use if-else chain (common for bool switches)
    if switch.selector.is_none() {
        // Build if-else chain: `if cond { body } else if cond { body } else { default }`
        if switch.cases.is_empty() && switch.default_stmts.is_empty() {
            return quote! { () };
        }

        // Handle the first case as the initial `if` (no `else` prefix)
        if !switch.cases.is_empty() {
            let first_case = &switch.cases[0];
            let first_conds: Vec<_> = first_case.exprs.iter().map(|e| go_to_rust(e)).collect();
            let first_body: Vec<_> = first_case.stmts.iter().map(|s| go_stmt_to_rust(s)).collect();
            let mut chain = quote! { if #(#first_conds)&&* { #(#first_body);* } };

            // Subsequent cases become `else if`
            for case in switch.cases.iter().skip(1) {
                if case.exprs.is_empty() {
                    continue;
                }
                let conds: Vec<_> = case.exprs.iter().map(|e| go_to_rust(e)).collect();
                let body: Vec<_> = case.stmts.iter().map(|s| go_stmt_to_rust(s)).collect();
                chain.extend(quote! { else if #(#conds)&&* { #(#body);* } });
            }

            // Default body as final `else`
            if !switch.default_stmts.is_empty() {
                let default_body: Vec<_> = switch.default_stmts.iter()
                    .map(|s| go_stmt_to_rust(s))
                    .collect();
                chain.extend(quote! { else { #(#default_body);* } });
            }

            return chain;
        }

        // No cases, only default
        if !switch.default_stmts.is_empty() {
            let db: Vec<_> = switch.default_stmts.iter()
                .map(|s| go_stmt_to_rust(s))
                .collect();
            return quote! { #(#db);* };
        }
        quote! { () }
    } else {
        // Build selector
        let selector = switch.selector.as_ref()
            .map(|s| go_to_rust(s))
            .unwrap_or_else(|| quote! { () });

        quote! { match #selector { #(#arms),* } }
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
