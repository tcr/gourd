//! Go statement to Rust token conversion (the `go_stmt_to_rust` bridge).

pub(crate) use super::ast::{GoForInit, GoStmt};
use super::expr::{dispatch, go_to_rust};
use super::types::map_go_types;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse_quote;

/// Convert a Go statement AST node to Rust tokens.
pub(crate) fn go_stmt_to_rust(stmt: &GoStmt) -> TokenStream {
    match stmt {
        GoStmt::Local(local) => {
            let pat = &local.pat;
            let val = local.init.as_ref().map(|v| go_to_rust(&v.expr));
            quote! { let #pat = #val; }
        }
        GoStmt::GoLocal(ident, val) => {
            quote! { let mut #ident = #val; }
        }
        GoStmt::If(go_if) => {
            let cond = go_to_rust(&go_if.cond);
            let then_body: Vec<_> = go_if.then_block.stmts.iter()
                .map(|s| go_stmt_to_rust(s)).collect();
            let then_block: Box<syn::ExprBlock> = syn::parse_quote!({ #(#then_body);* });
            let else_block = go_if.else_block.as_ref().map(|eb| {
                let else_body: Vec<_> = eb.stmts.iter().map(|s| go_stmt_to_rust(s)).collect();
                let block: Box<syn::ExprBlock> = syn::parse_quote!({ #(#else_body);* });
                quote! { else #block }
            });
            quote! { if #cond #then_block #else_block }
        }
        GoStmt::Expr(expr) => {
            go_to_rust(expr)
        }
        GoStmt::GoChannelSend(ch, val) => {
            let ch_rust = go_to_rust(ch);
            let val_rust = go_to_rust(val);
            quote! { #ch_rust.send(#val_rust); }
        }
        GoStmt::GoChannelRecv(ch) => {
            let ch_rust = go_to_rust(ch);
            quote! { return #ch_rust.recv().unwrap(); }
        }
        GoStmt::GoTypeAssert(receiver, ty) => {
            let recv_rust = go_to_rust(receiver);
            let ty_str = quote! { #ty }.to_string();
            match ty_str.as_str() {
                "String" => quote! { ::std::string::ToString::to_string(&#recv_rust) },
                "bool" => quote! { #recv_rust != 0 },
                "char" => quote! { (#recv_rust as u8) as char },
                _ => quote! { #recv_rust as #ty },
            }
        }
        GoStmt::GoMake(raw_args) => {
            go_go_make(raw_args)
        }
        GoStmt::GoSlice(elems) => {
            let elems: Vec<_> = elems.iter().map(go_to_rust).collect();
            quote! { vec![ #(#elems),* ] }
        }
        GoStmt::GoMap(ident, key_type, val_type, entries) => {
            go_stmt_to_rust_map(ident, key_type, val_type, entries)
        }
        GoStmt::GoReturn(exprs) => {
            if exprs.is_empty() {
                quote! { return }
            } else if exprs.len() == 1 {
                let e = &exprs[0];
                quote! { return #e }
            } else {
                let rust_exprs: Vec<_> = exprs.iter().map(go_to_rust).collect();
                quote! { return ( #(#rust_exprs),* ) }
            }
        }
        GoStmt::Switch(switch) => {
            super::free_fn::transpile_switch(switch)
        }
        GoStmt::Continue => {
            quote! { continue }
        }
        GoStmt::While(while_stmt) => {
            let cond = go_to_rust(&while_stmt.cond);
            let body: Vec<_> = while_stmt.body.stmts.iter()
                .map(|s| go_stmt_to_rust(s)).collect();
            quote! { while #cond { #(#body);* } }
        }
        GoStmt::GoFor(for_stmt) => {
            let body: Vec<_> = for_stmt.body.stmts.iter()
                .map(|s| go_stmt_to_rust(s)).collect();
            let body_block: Box<syn::ExprBlock> = syn::parse_quote!({ #(#body);* });

            match (&for_stmt.init, &for_stmt.is_range) {
                (Some(GoForInit::Double(i, v)), true) => {
                    let i_ident = i.clone();
                    let v_ident = v.clone();
                    let iterable = &for_stmt.iterable;
                    quote! {
                        for ( #i_ident, #v_ident ) in #iterable.iter().copied().enumerate() #body_block
                    }
                }
                (Some(GoForInit::Single(i)), true) => {
                    let i_ident = i.clone();
                    let iterable = &for_stmt.iterable;
                    quote! {
                        for #i_ident in 0.. #iterable.len() #body_block
                    }
                }
                (None, true) => {
                    let iterable = &for_stmt.iterable;
                    quote! {
                        for _ in 0.. #iterable.len() #body_block
                    }
                }
                _ => {
                    dispatch::emit_todo("unsupported for form")
                }
            }
        }
        GoStmt::RawStmt(tokens) => {
            tokens.clone()
        }
    }
}

/// Handle `make(...)` statements — channels, maps, slices.
fn go_go_make(raw_args: &str) -> TokenStream {
    let args_str = raw_args.trim().to_string();
    let normalized = args_str
        .replace(" [", "[")
        .replace(" ]", "]")
        .replace("  ", " ");

    if normalized.starts_with("chan ") {
        let chan_args: Vec<&str> = args_str.splitn(2, ',').collect();
        let chan_type_str = chan_args[0].trim().trim_start_matches("chan ").trim();
        let chan_type = super::types::map_go_type_str(chan_type_str);
        if chan_args.len() == 2 {
            let cap_str = chan_args[1].trim();
            let cap: TokenStream = parse_quote! { #cap_str };
            quote! { GoChannel::<#chan_type>::with_capacity(#cap) }
        } else {
            quote! { GoChannel::<#chan_type>::new() }
        }
    } else if normalized.starts_with("map[") {
        quote! { ::std::collections::HashMap::new() }
    } else if normalized.starts_with("[]") {
        let slice_args: Vec<&str> = normalized.splitn(2, ',').collect();
        let slice_type_str = slice_args[0].trim().trim_start_matches("[]").trim();
        let slice_type = super::types::map_go_type_str(slice_type_str);
        if slice_args.len() == 2 {
            let len_str = slice_args[1].trim();
            let len: TokenStream = parse_quote! { #len_str };
            quote! { ::std::iter::repeat(#slice_type::default()).take(#len).collect::<Vec::<#slice_type>>() }
        } else {
            quote! { ::std::iter::repeat(#slice_type::default()).take(0).collect::<Vec::<#slice_type>>() }
        }
    } else {
        quote! { { compile_error!(concat!("TODO: make with unsupported type: ", #args_str)) } }
    }
}

/// Handle `map[K]V{entries}` declarations.
fn go_stmt_to_rust_map(
    ident: &str,
    key_type: &Option<Box<syn::Type>>,
    val_type: &Option<Box<syn::Type>>,
    entries: &[(syn::Expr, syn::Expr)],
) -> TokenStream {
    if entries.is_empty() {
        if ident.is_empty() {
            return quote! { std::collections::HashMap::default() };
        }
        let name: syn::Ident = syn::parse_str(ident).unwrap();
        if let (Some(kt), Some(vt)) = (key_type, val_type) {
            let kt = map_go_types(kt);
            let vt = map_go_types(vt);
            return quote! { let #name = std::collections::HashMap::<#kt, #vt>::default(); };
        }
        return quote! { let #name = std::collections::HashMap::default(); };
    }

    let insertions: Vec<_> = entries.iter().map(|(k, v)| {
        let key = go_to_rust(k);
        let val = go_to_rust(v);
        quote! { m.insert(#key, #val); }
    }).collect();

    let block = if let (Some(kt), Some(vt)) = (key_type, val_type) {
        let kt = map_go_types(kt);
        let vt = map_go_types(vt);
        quote! {
            {
                let mut m = std::collections::HashMap::<#kt, #vt>::new();
                #(#insertions)*
                m
            }
        }
    } else {
        quote! {
            {
                let mut m = std::collections::HashMap::new();
                #(#insertions)*
                m
            }
        }
    };

    if ident.is_empty() {
        block
    } else {
        let name: syn::Ident = syn::parse_str(ident).unwrap();
        quote! { let #name = #block; }
    }
}
