//! Control-flow transpilation: `Let`, `Tuple`, `Return`, `Loop`, `ForLoop`,
//! `While`, `Range`, `If`, `Block`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{ExprBlock, ExprForLoop, ExprIf, ExprLoop, ExprMatch, ExprRange, ExprReturn, ExprTuple, ExprWhile};

pub fn transpile_let(input: &syn::ExprLet) -> TokenStream {
    let pat = &input.pat;
    let expr = crate::transpiler::legacy::expr_dispatch::go_to_rust(&input.expr);
    quote! { let #pat = #expr }
}

pub fn transpile_tuple(input: &ExprTuple) -> TokenStream {
    let elems: Vec<_> = input.elems.iter().map(crate::transpiler::legacy::expr_dispatch::go_to_rust).collect();
    match elems.len() {
        0 => quote! { () },
        _ => quote! { ( #(#elems),* ) },
    }
}

pub fn transpile_return(input: &ExprReturn) -> TokenStream {
    let expr = input.expr.as_ref().map(|e| crate::transpiler::legacy::expr_dispatch::go_to_rust(e));
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
    let expr = crate::transpiler::legacy::expr_dispatch::go_to_rust(&input.expr);
    let body = &input.body;
    quote! { for #pat in #expr #body }
}

pub fn transpile_while(input: &ExprWhile) -> TokenStream {
    let label = input.label.as_ref().map(|l| quote! { #l });
    let cond = crate::transpiler::legacy::expr_dispatch::go_to_rust(&input.cond);
    let body = &input.body;
    quote! { while #cond #label #body }
}

pub fn transpile_range(input: &ExprRange) -> TokenStream {
    let _start = input.start.as_ref().map(|e| crate::transpiler::legacy::expr_dispatch::go_to_rust(e));
    let end = input.end.as_ref().map(|e| crate::transpiler::legacy::expr_dispatch::go_to_rust(e));
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

fn transpile_pattern(pat: &syn::Pat) -> TokenStream {
    match pat {
        syn::Pat::Wild(_) => quote! { _ },
        syn::Pat::Ident(ident) => quote! { #ident },
        syn::Pat::Path(path) => quote! { #path },
        syn::Pat::Lit(lit) => {
            let lit = &lit.lit;
            quote! { #lit }
        }
        syn::Pat::Reference(r) => {
            let inner = transpile_pattern(&r.pat);
            quote! { &#inner }
        }
        syn::Pat::Tuple(tuple) => {
            let elems: Vec<_> = tuple.elems.iter().map(transpile_pattern).collect();
            quote! { ( #(#elems),* ) }
        }
        syn::Pat::TupleStruct(ts) => {
            let path = &ts.path;
            let elems: Vec<_> = ts.elems.iter().map(transpile_pattern).collect();
            quote! { #path ( #(#elems),* ) }
        }
        syn::Pat::Struct(s) => {
            let path = &s.path;
            let fields: Vec<_> = s.fields.iter().map(|f| {
                let name = &f.member;
                let pat = transpile_pattern(&f.pat);
                quote! { #name: #pat }
            }).collect();
            quote! { #path { #(#fields),* } }
        }
        _ => crate::transpiler::legacy::expr_dispatch::emit_todo("unsupported match pattern")
    }
}

pub fn transpile_match(input: &ExprMatch) -> TokenStream {
    let expr = crate::transpiler::legacy::expr_dispatch::go_to_rust(&input.expr);
    // Arm bodies are already processed by transpile_switch, so pass through
    // as raw tokens to avoid double-wrapping (e.g., String::from(String::from(...)))
    let arms: Vec<_> = input.arms.iter().map(|arm| {
        let pat = transpile_pattern(&arm.pat);
        let guard = arm.guard.as_ref().map(|(_, g)| {
            let g = crate::transpiler::legacy::expr_dispatch::go_to_rust(g);
            quote! { if #g }
        });
        let body: TokenStream = quote! { #&*arm.body };
        let comma = if arm.comma.is_some() { quote! { , } } else { quote! {} };
        quote! { #pat #guard => #body #comma }
    }).collect();
    quote! { match #expr { #(#arms)* } }
}

pub fn transpile_if(input: &ExprIf) -> TokenStream {
    let cond = crate::transpiler::legacy::expr_dispatch::go_to_rust(&input.cond);
    let then_block = &input.then_branch;
    let else_block = input.else_branch.as_ref().map(|(_, e)| {
        let e = crate::transpiler::legacy::expr_dispatch::go_to_rust(e);
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
                outputs.push(crate::transpiler::legacy::expr_dispatch::go_to_rust(val_expr));
            }
            syn::Stmt::Local(local) => {
                let local_pat = &local.pat;
                let local_val = local.init.as_ref().map(|v| crate::transpiler::legacy::expr_dispatch::go_to_rust(&v.expr));
                outputs.push(quote! { let #local_pat = #local_val; });
            }
            _ => return crate::transpiler::legacy::expr_dispatch::emit_todo("statement not yet supported"),
        }
    }
    quote! {{ { #(#outputs);* } }}
}
