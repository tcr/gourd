//! Statement block parsing: `parse_go_block`, special statements, block parsing.

pub(crate) use super::ast::{GoBlock, GoStmt};
use super::base_stmts::parse_base_stmt;
use super::control_flow::{parse_go_for, parse_go_if, parse_go_while};
use super::free_fn::select::parse_select_body;
use super::return_stmts::parse_go_return;
use super::slice_map::parse_go_slice_literal;
use super::switch::Switch;
use super::types::map_go_type_str;
use syn::ext::IdentExt;
use syn::parse::ParseStream;
use syn::token;
use syn::{Expr, Ident};

/// Parse a block of statements enclosed in braces.
pub(crate) fn parse_go_block(input: ParseStream) -> syn::Result<GoBlock> {
    let brace_content = if input.peek(syn::token::Brace) {
        // Standard case: `{` punctuation
        let content;
        let _brace = syn::braced!(content in input);
        content
    } else {
        // Handle Group token with Brace delimiter
        let tt: proc_macro2::TokenTree = input.parse()?;
        match tt {
            proc_macro2::TokenTree::Group(g) if g.delimiter() == proc_macro2::Delimiter::Brace => {
                // Parse the body from the Group's inner TokenStream
                return parse_body_from_group(&g.stream());
            }
            _ => {
                return Err(input.error("expected body `{`"));
            }
        }
    };

    let mut stmts = Vec::new();
    while !brace_content.is_empty() {
        if parse_go_special_stmt(&brace_content, &mut stmts)? {
            continue;
        }
        parse_base_stmt(&brace_content, &mut stmts)?;
    }

    Ok(GoBlock { stmts })
}

/// Parse body from a Group's TokenStream.
fn parse_body_from_group(ts: &proc_macro2::TokenStream) -> syn::Result<GoBlock> {
    let body_str = ts.to_string();
    
    // Split by `return` keyword to find statement boundaries
    let parts: Vec<&str> = if body_str.contains("return") {
        if let Some(pos) = body_str.find("return") {
            let before = body_str[..pos].trim();
            let from_return = body_str[pos..].trim();
            let mut p = Vec::new();
            if !before.is_empty() { p.push(before); }
            p.push(from_return);
            p
        } else {
            vec![body_str.trim()]
        }
    } else {
        vec![body_str.trim()]
    };
    
    let mut stmts = Vec::new();
    for part in &parts {
        let trimmed = part.trim();
        if trimmed.is_empty() { continue; }
        
        // Handle `return make(...)` specially (same as parse_go_return)
        if trimmed.starts_with("return make(") {
            let args_str = trimmed[11..].trim_end_matches(')').trim();
            // Determine the type of make call
            if args_str.starts_with("chan ") {
                let chan_args: Vec<&str> = args_str.trim_start_matches("chan ").trim().splitn(2, ',').collect();
                let chan_type_str = chan_args[0].trim();
                let chan_type = map_go_type_str(chan_type_str);
                if chan_args.len() > 1 {
                    let cap: syn::LitInt = syn::parse_str(chan_args[1].trim()).unwrap_or_else(
                        |_| syn::parse_quote!(0usize)
                    );
                    stmts.push(GoStmt::RawStmt(quote::quote! { return GoChannel::<#chan_type>::with_capacity(#cap) }));
                } else {
                    stmts.push(GoStmt::RawStmt(quote::quote! { return GoChannel::<#chan_type>::new() }));
                }
            } else if args_str.starts_with("map[") {
                stmts.push(GoStmt::RawStmt(quote::quote! { return ::std::collections::HashMap::new() }));
            } else if args_str.starts_with("[]") {
                let len: Option<syn::LitInt> = syn::parse_str(args_str.trim_start_matches("[]").trim()).ok();
                if let Some(l) = len {
                    stmts.push(GoStmt::RawStmt(quote::quote! { return vec![0; #l] }));
                } else {
                    stmts.push(GoStmt::RawStmt(quote::quote! { return Vec::new() }));
                }
            } else {
                // Unknown make type — fallback
                stmts.push(GoStmt::GoReturn(vec![]));
            }
        } else if trimmed.starts_with("return") {
            // Regular return statement
            let return_expr: Vec<syn::Expr> = trimmed[6..].trim()
                .split(',')
                .filter(|s| !s.is_empty())
                .filter_map(|s| syn::parse_str::<syn::Expr>(s.trim()).ok())
                .collect();
            stmts.push(GoStmt::GoReturn(return_expr));
        } else {
            // Try to parse as an expression (assignment, etc.)
            if let Some(go_stmt) = parse_stmt_from_str(trimmed) {
                stmts.push(go_stmt);
            }
        }
    }
    
    Ok(GoBlock { stmts })
}

/// Parse a Go statement from a string.
fn parse_stmt_from_str(s: &str) -> Option<GoStmt> {
    // Try to parse as an expression
    if let Ok(expr) = syn::parse_str::<syn::Expr>(s) {
        return Some(GoStmt::Expr(expr));
    }
    None
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
    // Handle `continue` — a reserved keyword, must be checked before ident block
    if input.peek(syn::token::Continue) {
        input.parse::<syn::token::Continue>()?;
        stmts.push(GoStmt::Continue);
        if input.peek(token::Semi) {
            let _semi: token::Semi = input.parse()?;
        }
        return Ok(true);
    }

    // Check for reserved keywords as identifiers (if, while, for)
    if input.peek(syn::Ident) || input.peek(syn::token::While) {
        let fork = input.fork();
        match fork.call(syn::Ident::parse_any) {
            Ok(kw) => {
                let kw_str = kw.to_string();
                if kw_str == "if" {
                    return parse_go_if(input, stmts);
                }

                // Check for while
                if kw_str == "while" {
                    let result = parse_go_while(input)?;
                    stmts.push(GoStmt::While(result));
                    if input.peek(token::Semi) {
                        let _semi: token::Semi = input.parse()?;
                    }
                    return Ok(true);
                }

                // Check for for
                if kw_str == "for" {
                    let result = parse_go_for(input)?;
                    stmts.push(GoStmt::GoFor(result));
                    if input.peek(token::Semi) {
                        let _semi: token::Semi = input.parse()?;
                    }
                    return Ok(true);
                }
            }
            Err(_) => {}
        }
    }

    // Check for `for` keyword directly (it's a Token::For, not an Ident)
    if input.peek(syn::token::For) {
        let result = parse_go_for(input)?;
        stmts.push(GoStmt::GoFor(result));
        if input.peek(token::Semi) {
            let _semi: token::Semi = input.parse()?;
        }
        return Ok(true);
    }

    // Check for switch

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
            // 7. Check for select
            if kw_str == "select" {
                let select_result = parse_select_body(input)?;
                stmts.push(GoStmt::Select(select_result));
                if input.peek(token::Semi) {
                    let _semi: token::Semi = input.parse()?;
                }
                return Ok(true);
            }
        }
    }

    // 8. Check for channel send: `chan <- value`
    //    Look ahead past the channel identifier to find `<-`
    if input.peek(syn::Ident) {
        let fork = input.fork();
        // Advance past identifier
        if let Ok(_id) = fork.parse::<syn::Ident>() {
            // Now check if the next token is `<`
            if fork.peek(syn::token::Lt) {
                let fork2 = fork.fork();
                if let Some((p, _)) = fork2.cursor().punct() {
                    if p.as_char() == '<' && p.spacing() == proc_macro2::Spacing::Joint {
                        // We have `chan <- ...` — parse it
                        let chan_ident: Ident = input.parse()?;
                        let chan_expr = Expr::Path(syn::ExprPath {
                            attrs: vec![],
                            qself: None,
                            path: syn::Path::from(chan_ident),
                        });
                        // Consume the `<-` operator
                        let _p1: proc_macro2::Punct = input.parse()?;
                        let _p2: proc_macro2::Punct = input.parse()?;
                        // Parse the value expression AFTER `<-`
                        let val_expr: Expr = input.parse()?;
                        stmts.push(GoStmt::GoChannelSend(chan_expr, val_expr));
                        if input.peek(token::Semi) {
                            let _semi: token::Semi = input.parse()?;
                        }
                        return Ok(true);
                    }
                }
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
