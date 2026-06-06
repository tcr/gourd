//! Statement block parsing: `parse_go_block`, special statements, block parsing.

pub(crate) use super::ast::{GoBlock, GoImport, GoStmt};
use super::base_stmts::parse_base_stmt;
use super::control_flow::{parse_go_for, parse_go_if, parse_go_while};
use super::free_fn::select::parse_select_body;
use super::return_stmts::parse_go_return;
use super::slice_map::parse_go_slice_literal;
use super::switch::Switch;
use super::types::map_go_type_str;
use proc_macro2::TokenStream;
use syn::ext::IdentExt;
use syn::parse::ParseStream;
use syn::token;
use syn::{Expr, Ident};

/// Parse a block of statements enclosed in braces.
pub(crate) fn parse_go_block(input: ParseStream) -> syn::Result<GoBlock> {
    eprintln!("[DEBUG parse_go_block] input.is_empty()={}, peek(Brace)={}", input.is_empty(), input.peek(syn::token::Brace));
    let brace_content = if input.peek(syn::token::Brace) {
        // Standard case: `{` punctuation
        eprintln!("[DEBUG parse_go_block] Entering standard brace branch");
        let content;
        let _brace = syn::braced!(content in input);
        eprintln!("[DEBUG parse_go_block] syn::braced! succeeded");
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
    
/// Parse body from a Group's TokenStream (debug version).
#[allow(dead_code)]
fn parse_body_from_group_debug(ts: &proc_macro2::TokenStream) -> syn::Result<GoBlock> {
    let body_str = ts.to_string();
    eprintln!("DEBUG parse_body_from_group_debug: body_str={}", body_str);
    parse_body_from_group(ts)
}

// Parse body from a Group's TokenStream.
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
                stmts.push(GoStmt::RawStmt(quote::quote! { return ::gourd::prelude::HashMap::new() }));
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

/// Parse a Go import declaration.
///
/// Supported forms:
/// - `import "strings"` → default alias (package name)
/// - `import s "strings"` → explicit alias
/// - `import . "fmt"` → dot import (makes all names visible)
/// - `import _ "os"` → blank import (side-effect only, no Rust output)
fn parse_go_import(input: ParseStream, stmts: &mut Vec<GoStmt>) -> syn::Result<bool> {
    // Consume `import`
    let _import: syn::Ident = input.parse()?;

    // Check for multi-import: `import ("os" "time")`
    if input.peek(syn::token::Paren) {
        let content;
        let _paren = syn::parenthesized!(content in input);
        while !content.is_empty() {
            let path: syn::LitStr = content.parse()?;
            // Each string in a multi-import is a simple import with default alias
            stmts.push(GoStmt::GoImport(GoImport {
                alias: None,
                dot: false,
                blank: false,
                path: path.value(),
            }));
            if content.peek(syn::token::Semi) {
                let _semi: token::Semi = content.parse()?;
            }
        }
        if input.peek(syn::token::Semi) {
            let _semi: token::Semi = input.parse()?;
        }
        return Ok(true);
    }

    // Single import forms: check for alias, dot, blank
    let fork = input.fork();
    let alias = if let Ok(ident) = fork.call(syn::Ident::parse_any) {
        let ident_str = ident.to_string();
        if ident_str == "." {
            // `import . "fmt"` — dot import
            let _dot: syn::Ident = input.parse()?;
            let path: syn::LitStr = input.parse()?;
            stmts.push(GoStmt::GoImport(GoImport {
                alias: None,
                dot: true,
                blank: false,
                path: path.value(),
            }));
            if input.peek(syn::token::Semi) {
                let _semi: token::Semi = input.parse()?;
            }
            return Ok(true);
        }
        if ident_str == "_" {
            // `import _ "os"` — blank import
            let _blank: syn::Ident = input.parse()?;
            let path: syn::LitStr = input.parse()?;
            stmts.push(GoStmt::GoImport(GoImport {
                alias: None,
                dot: false,
                blank: true,
                path: path.value(),
            }));
            if input.peek(syn::token::Semi) {
                let _semi: token::Semi = input.parse()?;
            }
            return Ok(true);
        }
        Some(ident)
    } else {
        None
    };

    // `import s "strings"` — aliased import
    if let Some(ident) = alias {
        let path: syn::LitStr = input.parse()?;
        stmts.push(GoStmt::GoImport(GoImport {
            alias: Some(ident),
            dot: false,
            blank: false,
            path: path.value(),
        }));
        if input.peek(syn::token::Semi) {
            let _semi: token::Semi = input.parse()?;
        }
        return Ok(true);
    }

    // `import "strings"` — default import (no alias)
    let path: syn::LitStr = input.parse()?;
    stmts.push(GoStmt::GoImport(GoImport {
        alias: None,
        dot: false,
        blank: false,
        path: path.value(),
    }));
    if input.peek(syn::token::Semi) {
        let _semi: token::Semi = input.parse()?;
    }
    Ok(true)
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

    // Check for reserved keywords as identifiers (if, while, for, import)
    if input.peek(syn::Ident) || input.peek(syn::token::While) {
        let fork = input.fork();
        match fork.call(syn::Ident::parse_any) {
            Ok(kw) => {
                let kw_str = kw.to_string();
                if kw_str == "import" {
                    return parse_go_import(input, stmts);
                }
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
            // 7b. Check for `defer` — Go's defer statement
            if kw_str == "defer" {
                let _defer: syn::Ident = input.parse()?;
                // Parse the closure body: `defer func() { ... }`
                let closure_body: TokenStream = input.parse()?;
                stmts.push(GoStmt::Defer(closure_body));
                if input.peek(token::Semi) {
                    let _semi: token::Semi = input.parse()?;
                }
                return Ok(true);
            }
            // 7c. Check for `if err != nil` pattern
            if kw_str == "if" {
                // Look ahead past `if` to detect `if err != nil`
                let fork = input.fork();
                if let Ok(_if) = fork.parse::<syn::Ident>() {
                    // Check for `err != nil` pattern
                    if fork.peek(syn::Ident) {
                        let err_fork = fork.fork();
                        if let Ok(err_name) = err_fork.parse::<syn::Ident>() {
                            let err_name_str = err_name.to_string();
                            // Check for `!=` operator (verify it's Bang followed by Eq)
                            if err_fork.peek2(syn::token::Eq) {
                                // We have `if <ident> != nil` — parse it
                                let _if: syn::Ident = input.parse()?;
                                let _err: syn::Ident = input.parse()?;
                                // Parse the error expression as the err variable
                                let err_expr: TokenStream = quote::quote! { #err_name_str };
                                // Parse the body block
                                let body_content;
                                syn::braced!(body_content in input);
                                let mut err_block = Vec::new();
                                while !body_content.is_empty() {
                                    if parse_go_special_stmt(&body_content, &mut err_block)? {
                                        continue;
                                    }
                                    parse_base_stmt(&body_content, &mut err_block)?;
                                }
                                stmts.push(GoStmt::GoIfErr(err_expr, err_block));
                                if input.peek(token::Semi) {
                                    let _semi: token::Semi = input.parse()?;
                                }
                                return Ok(true);
                            }
                        }
                    }
                }
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
