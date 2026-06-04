//! Control flow parsing: `if`, `for`, `while`.

use super::ast::{GoBlock, GoFor, GoForInit, GoIf, GoStmt, GoWhile};
use super::base_stmts::parse_base_stmt;
use syn::ext::IdentExt;
use syn::parse::{ParseStream, discouraged::Speculative};
use syn::{Expr};

/// Parse `while cond { body }`.
pub(crate) fn parse_go_while(input: ParseStream) -> syn::Result<GoWhile> {
    // Accept both Rust `while` token and Go `while` identifier
    if input.peek(syn::token::While) {
        input.parse::<syn::token::While>()?;
    } else if input.peek(syn::Ident) {
        let kw = input.call(syn::Ident::parse_any)?;
        if kw.to_string() == "while" {
            // matched
        } else {
            return Err(syn::Error::new(kw.span(), "expected 'while'"));
        }
    }
    let cond = input.parse::<Expr>()?;

    let body_content;
    let _brace = syn::braced!(body_content in input);

    let mut body_stmts = Vec::new();
    while !body_content.is_empty() {
        if super::stmts::parse_go_special_stmt(&body_content, &mut body_stmts)? {
            continue;
        }
        parse_base_stmt(&body_content, &mut body_stmts)?;
    }

    Ok(GoWhile {
        cond,
        body: GoBlock { stmts: body_stmts },
    })
}

/// Parse `for init := range iter { body }` or `for init, v := range iter { body }`.
pub(crate) fn parse_go_for(input: ParseStream) -> syn::Result<GoFor> {
    let _: syn::token::For = input.parse()?;

    let init = if input.peek(syn::Ident) {
        let fork = input.fork();
        if let Ok(first_ident) = fork.parse::<syn::Ident>() {
            if first_ident.to_string() == "range" {
                None
            } else {
                input.parse::<syn::Ident>()?;
                if input.peek(syn::token::Comma) {
                    let _: syn::token::Comma = input.parse()?;
                    let second_ident = input.parse::<syn::Ident>()?;
                    let _: syn::token::Colon = input.parse()?;
                    let _: syn::token::Eq = input.parse()?;
                    Some(GoForInit::Double(first_ident, second_ident))
                } else {
                    let _: syn::token::Colon = input.parse()?;
                    let _: syn::token::Eq = input.parse()?;
                    Some(GoForInit::Single(first_ident))
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    // Consume 'range' keyword
    if !matches!(&init, None) || input.peek(syn::Ident) {
        let fork = input.fork();
        match fork.parse::<syn::Ident>() {
            Ok(range_kw) => {
                if range_kw.to_string() == "range" {
                    let _: syn::Ident = input.parse()?;
                } else {
                    return Err(syn::Error::new(input.span(), "expected `range` keyword"));
                }
            }
            Err(_) => {
                return Err(syn::Error::new(input.span(), "expected `range` keyword"));
            }
        }
    }

    let iterable: syn::Path = input.parse()?;
    let body_content;
    let _brace = syn::braced!(body_content in input);
    let mut body_stmts = Vec::new();
    while !body_content.is_empty() {
        if super::stmts::parse_go_special_stmt(&body_content, &mut body_stmts)? {
            continue;
        }
        parse_base_stmt(&body_content, &mut body_stmts)?;
    }
    Ok(GoFor {
        init,
        is_range: true,
        iterable,
        body: GoBlock { stmts: body_stmts },
    })
}

/// Parse `if cond { body } else { ... }`.
pub(crate) fn parse_go_if(input: ParseStream, stmts: &mut Vec<GoStmt>) -> syn::Result<bool> {
    // Accept both `if` keyword (Rust) and `if` identifier (Go)
    if input.peek(syn::token::If) {
        input.parse::<syn::token::If>()?;
    } else if input.peek(syn::Ident) {
        let fork = input.fork();
        if let Ok(kw) = fork.parse::<syn::Ident>() {
            if kw.to_string() == "if" {
                input.parse::<syn::Ident>()?;
            }
        }
    }

    // Parse the condition expression — stop at `{` because syn's Expr::parse
    // treats `a > b { ... }` as a field access on the binary expression.
    // We use a fork-to-brace approach instead.
    let cond_fork = input.fork();
    // Collect tokens into a separate stream (stop at `{`)
    let mut cond_tokens: proc_macro2::TokenStream = proc_macro2::TokenStream::new();
    while !cond_fork.is_empty() && !cond_fork.peek(syn::token::Brace) {
        if let Ok(tt) = cond_fork.parse::<proc_macro2::TokenTree>() {
            cond_tokens.extend(std::iter::once(tt));
        } else {
            break;
        }
    }
    // Try parsing as Expr first
    let cond_tokens2 = cond_tokens.clone();
    let cond_tokens3 = cond_tokens.clone();
    let cond: Expr = if let Ok(e) = syn::parse2::<Expr>(cond_tokens) {
        e
    } else if let Ok(e) = syn::parse2::<Expr>(cond_tokens2) {
        e
    } else {
        // Fall back to Verbatim
        syn::Expr::Verbatim(cond_tokens3)
    };
    input.advance_to(&cond_fork);

    let then_block_content;
    let _brace = syn::braced!(then_block_content in input);
    let then_block = super::stmts::parse_block_stmts(&then_block_content)?;

    let else_block = if input.peek(syn::token::Else) {
        input.parse::<syn::token::Else>()?;
        if input.peek(syn::token::Brace) {
            let else_block_content;
            let _brace = syn::braced!(else_block_content in input);
            Some(GoBlock { stmts: super::stmts::parse_block_stmts(&else_block_content)? })
        } else {
            None
        }
    } else {
        None
    };

    stmts.push(GoStmt::If(GoIf {
        cond,
        then_block: GoBlock { stmts: then_block },
        else_block,
    }));
    Ok(true)
}
