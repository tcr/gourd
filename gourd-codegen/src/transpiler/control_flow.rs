//! Control flow parsing: `if`, `for`, `while`.

use super::ast::{GoBlock, GoFor, GoForInit, GoIf, GoStmt, GoWhile};
use super::base_stmts::parse_base_stmt;
use syn::parse::ParseStream;
use syn::{Expr};

/// Parse `while cond { body }`.
pub(crate) fn parse_go_while(input: ParseStream) -> syn::Result<GoWhile> {
    let _: syn::token::While = input.parse()?;
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
    input.parse::<syn::token::If>()?;
    let cond: Expr = input.parse()?;
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
