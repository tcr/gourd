//! Statement block parsing: `parse_go_block`, special statements, block parsing.

pub(crate) use super::ast::{GoBlock, GoStmt};
use super::base_stmts::parse_base_stmt;
use super::control_flow::{parse_go_for, parse_go_if, parse_go_while};
use super::return_stmts::parse_go_return;
use super::slice_map::parse_go_slice_literal;
use super::switch::Switch;
use syn::parse::discouraged::Speculative;
use syn::parse::ParseStream;
use syn::token;
use syn::Expr;

/// Parse a block of statements enclosed in braces.
pub(crate) fn parse_go_block(input: ParseStream) -> syn::Result<GoBlock> {
    let brace_content;
    let _brace = syn::braced!(brace_content in input);

    let mut stmts = Vec::new();
    while !brace_content.is_empty() {
        if parse_go_special_stmt(&brace_content, &mut stmts)? {
            continue;
        }
        parse_base_stmt(&brace_content, &mut stmts)?;
    }

    Ok(GoBlock { stmts })
}

/// Try to parse a Go-specific statement. Returns `true` if consumed.
pub(crate) fn parse_go_special_stmt(input: ParseStream, stmts: &mut Vec<GoStmt>) -> syn::Result<bool> {
    // 1. Check for Go slice literal: `[]...{...}`
    if input.peek(syn::token::Bracket) {
        if let Ok(()) = parse_go_slice_literal(input, stmts) {
            return Ok(true);
        }
    }

    // 2. Check for if statement
    if input.peek(syn::token::If) {
        return parse_go_if(input, stmts);
    }
    if input.peek(syn::token::Return) {
        return parse_go_return(input, stmts);
    }
    if input.peek(syn::Ident) {
        let fork = input.fork();
        match fork.parse::<syn::Ident>() {
            Ok(kw) => {
                let kw_str = kw.to_string();
                if kw_str == "if" {
                    return parse_go_if(input, stmts);
                }

                // 3. Check for while
                if kw_str == "while" {
                    let result = parse_go_while(input)?;
                    stmts.push(GoStmt::While(result));
                    if input.peek(token::Semi) {
                        let _semi: token::Semi = input.parse()?;
                    }
                    return Ok(true);
                }

                // 4. Check for for
                if kw_str == "for" {
                    let result = parse_go_for(input)?;
                    stmts.push(GoStmt::GoFor(result));
                    if input.peek(token::Semi) {
                        let _semi: token::Semi = input.parse()?;
                    }
                    return Ok(true);
                }

                // 5. Check for continue (continue is a Rust keyword)
                if kw_str == "continue" {
                    stmts.push(GoStmt::Continue);
                    if input.peek(token::Semi) {
                        let _semi: token::Semi = input.parse()?;
                    }
                    return Ok(true);
                }
            }
            Err(_) => {}
        }
    }

    // 6. Check for switch
    if input.peek(syn::Ident) {
        let fork = input.fork();
        if let Ok(kw) = fork.parse::<syn::Ident>() {
            let kw_str = kw.to_string();
            if kw_str == "switch" {
                let sw: Switch = input.parse()?;
                stmts.push(GoStmt::Switch(sw));
                if input.peek(token::Semi) {
                    let _semi: token::Semi = input.parse()?;
                }
                return Ok(true);
            }
        }
    }

    // 8. Check for channel send: `ch <- value`
    if input.peek(syn::token::Lt) || input.peek(syn::token::Le) {
        let fork = input.fork();
        if let Some((p, _)) = fork.cursor().punct() {
            if p.as_char() == '<' && p.spacing() == proc_macro2::Spacing::Joint {
                input.advance_to(&fork);
                let _p1: proc_macro2::Punct = input.parse()?;
                let _p2: proc_macro2::Punct = input.parse()?;
                let val_expr: Expr = input.parse()?;
                let chan_expr = Expr::Path(syn::ExprPath {
                    attrs: vec![],
                    qself: None,
                    path: syn::Path::from(syn::Ident::new("ch", proc_macro2::Span::call_site())),
                });
                stmts.push(GoStmt::GoChannelSend(chan_expr, val_expr));
                if input.peek(token::Semi) {
                    let _semi: token::Semi = input.parse()?;
                }
                return Ok(true);
            }
        }
    }

    Ok(false)
}

/// Parse statements from a ParseStream without consuming braces.
pub(crate) fn parse_block_stmts(input: ParseStream) -> syn::Result<Vec<GoStmt>> {
    let mut stmts = Vec::new();
    while !input.is_empty() {
        if parse_go_special_stmt(input, &mut stmts)? {
            continue;
        }
        parse_base_stmt(input, &mut stmts)?;
    }
    Ok(stmts)
}
