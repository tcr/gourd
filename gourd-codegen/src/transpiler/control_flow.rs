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

/// Parse `for init := range iter { body }`, `for init, v := range iter { body }`,
/// or C-style `for i := 0; i < n; i++ { body }`.
pub(crate) fn parse_go_for(input: ParseStream) -> syn::Result<GoFor> {
    let _: syn::token::For = input.parse()?;

    // Parse optional init (either `i := 0` or nothing for C-style loops)
    let init = if input.peek(syn::Ident) {
        let fork = input.fork();
        if let Ok(first_ident) = fork.parse::<syn::Ident>() {
            if first_ident.to_string() == "range" {
                None
            } else {
                let parsed_ident = input.parse::<syn::Ident>()?;
                if input.peek(syn::token::Comma) {
                    let _: syn::token::Comma = input.parse()?;
                    let second_ident = input.parse::<syn::Ident>()?;
                    let _: syn::token::Colon = input.parse()?;
                    let _: syn::token::Eq = input.parse()?;
                    // Parse the init value (e.g., `0` in `i := 0`)
                    // Range loops don't have an init value; they have `range` after `:=`
                    let init_val: Option<Box<syn::Expr>> = if input.peek(syn::token::Semi) || input.peek(syn::Ident) {
                        None // No init value (range loop or C-style without init)
                    } else {
                        Some(Box::new(input.parse()?))
                    };
                    Some(GoForInit::Double(parsed_ident, second_ident, init_val))
                } else {
                    let _: syn::token::Colon = input.parse()?;
                    let _: syn::token::Eq = input.parse()?;
                    // Parse the init value (e.g., `0` in `i := 0`)
                    // Range loops don't have an init value; they have `range` after `:=`
                    let init_val: Option<Box<syn::Expr>> = if input.peek(syn::token::Semi) || input.peek(syn::Ident) {
                        None
                    } else {
                        Some(Box::new(input.parse()?))
                    };
                    Some(GoForInit::Single(parsed_ident, init_val))
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    // Detect C-style `for` vs `for` with `range`
    let is_range = if input.peek(syn::Ident) {
        let fork = input.fork();
        if let Ok(range_kw) = fork.parse::<syn::Ident>() {
            if range_kw.to_string() == "range" {
                let _: syn::Ident = input.parse()?;
                true
            } else {
                false
            }
        } else {
            false
        }
    } else if input.peek(syn::token::Semi) {
        false // C-style: no range keyword
    } else {
        true // default to range
    };

    if is_range {
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
        let body: GoBlock = GoBlock { stmts: body_stmts };
        Ok(GoFor {
            init,
            is_range: true,
            iterable: Some(iterable),
            cond: None,
            post: None,
            body,
        })
    } else {
        // C-style `for` loop: `for init; cond; post { body }`
        let mut cond: Option<Box<syn::Expr>> = None;
        let mut post: Option<Box<syn::Expr>> = None;

        if input.peek(syn::token::Semi) {
            let _: syn::token::Semi = input.parse()?;
            if !input.peek(syn::token::Semi) && !input.peek(syn::token::Brace) {
                // Parse condition. When no post statement follows, the condition
                // is followed directly by {. Comparison operators (<, <=, >=,
                // >, ==, !=) are ambiguous in this context because syn tries
                // to parse beyond the condition into the brace group.
                // For this case, parse the condition using input.parse() which
                // works when the condition is followed by a delimiter (; or {}).
                // When followed by { (no post), the < operator is ambiguous.
                // In that case, treat as a while-loop pattern instead.
                let expr: syn::Expr = input.parse()?;
                cond = Some(Box::new(expr));
            }
            if input.peek(syn::token::Semi) {
                let _: syn::token::Semi = input.parse()?;
                if !input.peek(syn::token::Brace) {
                    let expr: syn::Expr = input.parse()?;
                    post = Some(Box::new(expr));
                }
            }
        }

        let body_content;
        let _brace = syn::braced!(body_content in input);
        let mut body_stmts = Vec::new();
        while !body_content.is_empty() {
            if super::stmts::parse_go_special_stmt(&body_content, &mut body_stmts)? {
                continue;
            }
            parse_base_stmt(&body_content, &mut body_stmts)?;
        }
        let body: GoBlock = GoBlock { stmts: body_stmts };
        Ok(GoFor {
            init,
            is_range: false,
            iterable: None,
            cond,
            post,
            body,
        })
    }
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





