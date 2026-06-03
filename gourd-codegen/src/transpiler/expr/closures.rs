//! Go anonymous function (closure) transpilation.
//!
//! Go anonymous functions: `func(params) ret { body }`
//! Rust closures: `|params| -> ret { body }`

use proc_macro2::{TokenStream, TokenTree};
use quote::quote;
use syn::{Expr, Ident};

/// Parsed Go anonymous function.
pub(crate) struct GoClosure {
    pub(crate) params: Vec<GoClosureParam>,
    pub(crate) output: Option<syn::Type>,
    pub(crate) body: GoBlock,
}

pub(crate) struct GoClosureParam {
    pub(crate) id: Ident,
    pub(crate) ty: Box<syn::Type>,
}

#[derive(Default)]
pub(crate) struct GoBlock {
    pub(crate) stmts: Vec<GoStmt>,
}

pub(crate) enum GoStmt {
    Local(syn::Local),
    Expr(syn::Expr),
    Return(Vec<Expr>),
    Break,
    Continue,
    RawStmt(TokenStream),
    GoBlock(GoBlock), // nested block
}

/// Parse an anonymous Go function: `func(params) ret { body }`.
///
/// Called when we detect `func` keyword followed by a parenthesized
/// parameter list and a brace-delimited body — but without a name
/// (hence "anonymous").
pub(crate) fn parse_closure(input: &TokenStream) -> Option<GoClosure> {
    // Must start with `func`
    let trees: Vec<TokenTree> = input.clone().into_iter().collect();
    let mut i = 0;

    if i >= trees.len() {
        return None;
    }
    if let TokenTree::Ident(id) = &trees[i] {
        if id.to_string() != "func" {
            return None;
        }
    } else {
        return None;
    }
    i += 1;

    // Parse parameter list in parentheses
    let mut params = Vec::new();
    if i >= trees.len() {
        return None;
    }
    if let TokenTree::Group(g) = &trees[i] {
        if g.delimiter() == proc_macro2::Delimiter::Parenthesis {
            let param_tokens: Vec<TokenTree> = g.stream().into_iter().collect();
            params = parse_closure_params(&param_tokens)?;
            i += 1;
        } else {
            return None;
        }
    } else {
        return None;
    }

    // Optional return type: either an ident or a grouped type
    let output = if i < trees.len() {
        let next = &trees[i];
        match next {
            TokenTree::Ident(id) => {
                let type_name = id.to_string();
                if is_go_type_name(&type_name) {
                    i += 1;
                    Some(syn::parse_str(&type_name).ok())
                } else {
                    None
                }
            }
            TokenTree::Group(g) => {
                let type_tokens: TokenStream = g.stream();
                Some(syn::parse2::<syn::Type>(type_tokens).ok())
            }
            _ => None,
        }
    } else {
        None
    };

    // Body in braces
    if i >= trees.len() {
        return None;
    }
    if let TokenTree::Group(g) = &trees[i] {
        if g.delimiter() == proc_macro2::Delimiter::Brace {
            let body_tokens: Vec<TokenTree> = g.stream().into_iter().collect();
            let body = parse_block(&body_tokens);
            return Some(GoClosure { params, output: output.flatten(), body });
        }
    }

    None
}

/// Parse closure parameters: `a int, b int` or `a, b int`.
fn parse_closure_params(trees: &[TokenTree]) -> Option<Vec<GoClosureParam>> {
    let mut params = Vec::new();
    let mut i = 0;

    while i < trees.len() {
        // Skip commas
        if let TokenTree::Punct(p) = &trees[i] {
            if p.as_char() == ',' {
                i += 1;
                continue;
            }
        }

        // Parse identifier
        if let TokenTree::Ident(id) = &trees[i] {
            i += 1;

            // Check if next token is a type
            if i < trees.len() {
                let next = &trees[i];
                if let TokenTree::Ident(type_id) = next {
                    let type_name = type_id.to_string();
                    if is_go_type_name(&type_name) {
                        let ty = syn::parse_str(&type_name).ok()?;
                        params.push(GoClosureParam {
                            id: id.clone(),
                            ty: Box::new(ty),
                        });
                        i += 1;
                        continue;
                    }
                }
                // Group type (e.g., `[]int`)
                if let TokenTree::Group(g) = next {
                    if g.delimiter() == proc_macro2::Delimiter::Bracket {
                        let type_tokens: TokenStream = g.stream();
                        let ty = syn::parse2::<syn::Type>(type_tokens).ok()?;
                        params.push(GoClosureParam {
                            id: id.clone(),
                            ty: Box::new(ty),
                        });
                        i += 1;
                        continue;
                    }
                }
            }
        }

        // Skip unknown tokens to advance
        i += 1;
    }

    if params.is_empty() {
        None
    } else {
        Some(params)
    }
}

/// Check if a string is a Go type name.
fn is_go_type_name(name: &str) -> bool {
    matches!(name,
        "int" | "int8" | "int16" | "int32" | "int64"
        | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr"
        | "byte" | "rune"
        | "float32" | "float64"
        | "string" | "bool" | "error"
    )
}

/// Parse a block of statements inside a closure body.
fn parse_block(trees: &[TokenTree]) -> GoBlock {
    use syn::Stmt;

    let mut stmts = Vec::new();

    // Combine tokens back into a stream for syn parsing
    let token_stream: TokenStream = trees.iter().cloned().collect();

    // Try to parse as an expression block
    if let Ok(block) = syn::parse2::<syn::ExprBlock>(token_stream) {
        for stmt in block.block.stmts {
            match stmt {
                Stmt::Local(local) => {
                    stmts.push(GoStmt::Local(local));
                }
                Stmt::Expr(expr, _) => {
                    stmts.push(GoStmt::Expr(expr));
                }
                Stmt::Item(_) | Stmt::Macro(_) => {
                    // Skip items and macros
                }
            }
        }
    }

    GoBlock { stmts }
}

/// Convert a Go closure to Rust closure tokens.
pub(crate) fn closure_to_rust(closure: &GoClosure) -> TokenStream {
    use super::super::types::map_go_types;
    use super::dispatch::go_to_rust;

    // Build closure parameters
    let rust_params: Vec<TokenStream> = closure.params.iter().map(|p| {
        let id = &p.id;
        let ty = map_go_types(&p.ty);
        quote! { #id: #ty }
    }).collect();

    // Build closure body
    let body_stmts: Vec<TokenStream> = closure.body.stmts.iter().map(|s| {
        match s {
            GoStmt::Local(local) => {
                let pat = &local.pat;
                let val = local.init.as_ref().map(|v| go_to_rust(&v.expr));
                quote! { let #pat = #val; }
            }
            GoStmt::Expr(expr) => go_to_rust(expr),
            GoStmt::Return(exprs) => {
                let rust_exprs: Vec<_> = exprs.iter().map(go_to_rust).collect();
                if rust_exprs.is_empty() {
                    quote! { return; }
                } else if rust_exprs.len() == 1 {
                    quote! { return #(#rust_exprs)*; }
                } else {
                    quote! { return ( #(#rust_exprs),* ); }
                }
            }
            GoStmt::Break => quote! { break; },
            GoStmt::Continue => quote! { continue; },
            GoStmt::RawStmt(tokens) => tokens.clone(),
            GoStmt::GoBlock(block) => {
                let inner_stmts: Vec<_> = block.stmts.iter().map(|s| {
                    match s {
                        GoStmt::Local(local) => {
                            let pat = &local.pat;
                            let val = local.init.as_ref().map(|v| go_to_rust(&v.expr));
                            quote! { let #pat = #val; }
                        }
                        GoStmt::Expr(expr) => go_to_rust(expr),
                        _ => quote! { /* unsupported */ },
                    }
                }).collect();
                quote! { { #(#inner_stmts);* } }
            }
        }
    }).collect();

    // Build return type if present
    let ret = closure.output.as_ref().map(|ty| {
        let mapped = map_go_types(ty);
        quote! { -> #mapped }
    });

    quote! { | #(#rust_params),* #ret { #(#body_stmts);* } }
}
