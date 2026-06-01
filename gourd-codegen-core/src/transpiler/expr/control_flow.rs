//! Control-flow transpilation: `Let`, `Tuple`, `Return`, `Loop`, `ForLoop`,
//! `While`, `Range`, `If`, `Block`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{ExprBlock, ExprForLoop, ExprIf, ExprLoop, ExprRange, ExprReturn, ExprTuple, ExprWhile};

pub fn transpile_let(input: &syn::ExprLet) -> TokenStream {
    let pat = &input.pat;
    let expr = super::dispatch::go_to_rust(&input.expr);
    quote! { let #pat = #expr }
}

pub fn transpile_tuple(input: &ExprTuple) -> TokenStream {
    let elems: Vec<_> = input.elems.iter().map(super::dispatch::go_to_rust).collect();
    match elems.len() {
        0 => quote! { () },
        _ => quote! { ( #(#elems),* ) },
    }
}

pub fn transpile_return(input: &ExprReturn) -> TokenStream {
    let expr = input.expr.as_ref().map(|e| super::dispatch::go_to_rust(e));
    match expr {
        Some(e) => quote! { return #e },
        None => quote! { return },
    }
}

pub fn transpile_loop(input: &ExprLoop) -> TokenStream {
    let label = input.label.as_ref().map(|l| quote! { #l });
    let body = &input.body;
    quote! { loop #label #body }
}

pub fn transpile_for_loop(input: &ExprForLoop) -> TokenStream {
    let pat = &input.pat;
    let expr = super::dispatch::go_to_rust(&input.expr);
    let body = &input.body;
    quote! { for #pat in #expr #body }
}

pub fn transpile_while(input: &ExprWhile) -> TokenStream {
    let label = input.label.as_ref().map(|l| quote! { #l });
    let cond = super::dispatch::go_to_rust(&input.cond);
    let body = &input.body;
    quote! { while #cond #label #body }
}

pub fn transpile_range(input: &ExprRange) -> TokenStream {
    let _start = input.start.as_ref().map(|e| super::dispatch::go_to_rust(e));
    let end = input.end.as_ref().map(|e| super::dispatch::go_to_rust(e));
    let limits = match input.limits {
        syn::RangeLimits::HalfOpen(_) => quote! { .. },
        syn::RangeLimits::Closed(_)   => quote! { ..= },
    };
    match (input.start.as_ref(), input.end.as_ref()) {
        (Some(fd), Some(_ld))  => quote! { #fd #limits #end },
        (Some(e), None)        => quote! { #e #limits },
        (None, Some(e))        => quote! { #limits #e },
        (None, None)           => quote! { #limits },
    }
}

pub fn transpile_if(input: &ExprIf) -> TokenStream {
    let cond = super::dispatch::go_to_rust(&input.cond);
    let then_block = &input.then_branch;
    let else_block = input.else_branch.as_ref().map(|(_, e)| {
        let e = super::dispatch::go_to_rust(e);
        quote! { else { #e } }
    });
    quote! { if #cond #then_block #else_block }
}

pub fn transpile_block(input: &ExprBlock) -> TokenStream {
    if input.block.stmts.is_empty() {
        return quote! {{ }};
    }
    let mut outputs = Vec::new();
    for stm in input.block.stmts.iter() {
        match stm {
            syn::Stmt::Expr(val_expr, _) => {
                outputs.push(super::dispatch::go_to_rust(val_expr));
            }
            syn::Stmt::Local(local) => {
                let local_pat = &local.pat;
                let local_val = local.init.as_ref().map(|v| super::dispatch::go_to_rust(&v.expr));
                outputs.push(quote! { let #local_pat = #local_val; });
            }
            _ => return super::dispatch::emit_todo("statement not yet supported"),
        }
    }
    quote! {{ { #(#outputs);* } }}
}
