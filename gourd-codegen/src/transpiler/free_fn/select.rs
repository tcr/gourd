//! Select statement transpilation.
//!
//! Converts Go `select { case ... default: ... }` to Rust `GoSelect::new()`
//! with proper case building using `send_case()`, `recv_case()`, and
//! `with_default()`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::discouraged::Speculative;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Ident};

use super::super::ast::{GoBlock, GoSelect, GoSelectCase};
use super::super::expr::go_to_rust;

// ─── Parse implementations ─────────────────────────────────────────────────

/// Parse a `GoSelect` from an input stream.
///
/// Expects: `select { case ... default: ... }`
impl Parse for GoSelect {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let brace_content;
        syn::braced!(brace_content in input);

        let mut cases: Vec<GoSelectCase> = Vec::new();
        while !brace_content.is_empty() {
            let case_fork = brace_content.fork();
            if case_fork.peek(syn::Ident) {
                let kw_fork = case_fork.fork();
                if let Ok(kw) = kw_fork.parse::<syn::Ident>() {
                    let kw_str = kw.to_string();
                    if kw_str == "case" {
                        brace_content.advance_to(&case_fork);
                        let case_result: GoSelectCase = brace_content.parse()?;
                        cases.push(case_result);
                        continue;
                    }
                    if kw_str == "default" {
                        brace_content.advance_to(&case_fork);
                        brace_content.parse::<syn::Ident>()?;
                        if brace_content.peek(syn::token::Colon) {
                            let _: syn::token::Colon = brace_content.parse()?;
                        }
                        let block: GoBlock = if brace_content.peek(syn::token::Brace) {
                            brace_content.parse()?
                        } else {
                            GoBlock::default()
                        };
                        cases.push(GoSelectCase::Default(block));
                        continue;
                    }
                }
            }
            // Skip unknown tokens
            let _ = brace_content.parse::<proc_macro2::TokenTree>();
        }

        Ok(GoSelect { cases })
    }
}

/// Parse a single select case.
impl Parse for GoSelectCase {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Parse `case` keyword
        let case_fork = input.fork();
        if let Ok(kw) = case_fork.parse::<syn::Ident>() {
            let kw_str = kw.to_string();
            if kw_str == "case" {
                input.parse::<syn::Ident>()?;
            }
        }

        // Determine case type
        let fork = input.fork();

        // Check for send case: `ident <- expr`
        if is_send_case(&fork) {
            return parse_send_case(input);
        }

        // Check for recv case: `<-expr`
        if is_recv_case(&fork) {
            return parse_recv_case(input);
        }

        // Fallback: skip until colon
        skip_to_colon(input)?;
        Ok(GoSelectCase::Default(GoBlock::default()))
    }
}

/// Check if input looks like a send case (`ident <- expr`).
fn is_send_case(fork: &syn::parse::ParseBuffer) -> bool {
    let fork = fork.fork();
    if fork.peek(syn::Ident) {
        let fork = fork.fork();
        if fork.parse::<syn::Ident>().is_ok() {
            let fork = fork.fork();
            if fork.peek(syn::token::Lt) {
                return true;
            }
        }
    }
    false
}

/// Check if input looks like a recv case (`<-expr`).
fn is_recv_case(fork: &syn::parse::ParseBuffer) -> bool {
    fork.peek(syn::token::Lt)
}

/// Parse a send case: `ch <- value`
///
/// IMPORTANT: We cannot use `Expr::parse` directly because `syn` would parse
/// `ch <- value` as a binary expression (`ch < -value`). Instead, we parse the
/// channel expression up to the first `<` punctuation, then parse the value
/// expression after `<-`.
fn parse_send_case(input: ParseStream) -> syn::Result<GoSelectCase> {
    // Parse the channel expression manually by collecting tokens until `<`
    let mut chan_tokens = proc_macro2::TokenStream::new();
    let mut value_tokens = proc_macro2::TokenStream::new();
    let mut in_value = false;

    loop {
        if input.is_empty() {
            break;
        }
        let token: proc_macro2::TokenTree = input.parse()?;
        if !in_value {
            if let proc_macro2::TokenTree::Punct(punct) = &token {
                if punct.as_char() == '<' {
                    // Check if next token is `-` (making it `<-`)
                    if input.peek(syn::token::Minus) {
                        in_value = true;
                        // Consume the `-`
                        let _minus: proc_macro2::Punct = input.parse()?;
                        // Don't add `<` to chan_tokens - it's part of `<-`
                        continue;
                    }
                    // If not followed by `-`, continue collecting
                }
            }
            chan_tokens.extend(std::iter::once(token));
        } else {
            // When in value mode, stop at `:` (case delimiter)
            if let proc_macro2::TokenTree::Punct(punct) = &token {
                if punct.as_char() == ':' {
                    break;
                }
            }
            value_tokens.extend(std::iter::once(token));
        }
    }

    // Parse channel expression from collected tokens
    let chan_expr: Expr = syn::parse2(chan_tokens)?;
    let chan_rust = go_to_rust(&chan_expr);

    // Parse value expression from collected tokens
    let val_expr: Expr = syn::parse2(value_tokens)?;
    let val_rust = go_to_rust(&val_expr);

    Ok(GoSelectCase::Send {
        ch: Box::new(chan_rust),
        value: Box::new(val_rust),
    })
}

/// Parse a receive case: `<-ch`
fn parse_recv_case(input: ParseStream) -> syn::Result<GoSelectCase> {
    // Parse `<-` operator (two separate puncts: `<` then `-`)
    if input.peek(syn::token::Lt) {
        let _: proc_macro2::Punct = input.parse()?; // consume `<`
        if input.peek(syn::token::Minus) {
            let _: proc_macro2::Punct = input.parse()?; // consume `-`
        }
    }

    // Parse channel expression
    let chan_expr: Expr = input.parse()?;
    let chan_rust = go_to_rust(&chan_expr);

    Ok(GoSelectCase::Recv {
        ch: Box::new(chan_rust),
        target: None,
    })
}

/// Skip tokens until we hit a colon.
fn skip_to_colon(input: ParseStream) -> syn::Result<()> {
    loop {
        if input.peek(syn::token::Colon) {
            let _: syn::token::Colon = input.parse()?;
            return Ok(());
        }
        let _ = input.parse::<proc_macro2::TokenTree>();
        if input.is_empty() {
            break;
        }
    }
    Ok(())
}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Parse a select statement body from the input stream.
///
/// Parses the `select { ... }` form and extracts all cases:
/// - `case ch <- value:` → send case
/// - `case <-ch:` → receive case
/// - `default:` → default case
pub(crate) fn parse_select_body(input: ParseStream) -> syn::Result<GoSelect> {
    let mut cases = Vec::new();

    // Consume the `select` keyword
    let select_fork = input.fork();
    if let Ok(kw) = select_fork.parse::<syn::Ident>() {
        let kw_str = kw.to_string();
        if kw_str == "select" {
            input.parse::<syn::Ident>()?;
        }
    }

    // Consume the brace-delimited body
    let brace_content;
    syn::braced!(brace_content in input);

    // Parse each case line inside the select body
    while !brace_content.is_empty() {
        let case_fork = brace_content.fork();
        if case_fork.peek(syn::Ident) {
            let kw_fork = case_fork.fork();
            if let Ok(kw) = kw_fork.parse::<syn::Ident>() {
                let kw_str = kw.to_string();
                if kw_str == "case" {
                    brace_content.advance_to(&case_fork);
                    let case_result: GoSelectCase = brace_content.parse()?;
                    cases.push(case_result);
                    continue;
                }
            }
        }

        let default_fork = brace_content.fork();
        if default_fork.peek(syn::Ident) {
            let kw_fork = default_fork.fork();
            if let Ok(kw) = kw_fork.parse::<syn::Ident>() {
                let kw_str = kw.to_string();
                if kw_str == "default" {
                    brace_content.advance_to(&default_fork);
                    brace_content.parse::<syn::Ident>()?;
                    if brace_content.peek(syn::token::Colon) {
                        let _: syn::token::Colon = brace_content.parse()?;
                    }
                    let block: GoBlock = if brace_content.peek(syn::token::Brace) {
                        brace_content.parse()?
                    } else {
                        GoBlock::default()
                    };
                    cases.push(GoSelectCase::Default(block));
                    continue;
                }
            }
        }

        let _ = brace_content.parse::<proc_macro2::TokenTree>();
    }

    Ok(GoSelect { cases })
}

/// Transpile a raw token stream into a select statement.
///
/// For backward compatibility with the old API that took raw tokens.
pub fn go_to_rust_select(input: TokenStream) -> TokenStream {
    match syn::parse2::<GoSelect>(input) {
        Ok(select) => go_to_rust_select_ast(&select),
        Err(_) => {
            // Fallback: legacy path — wrap body in GoSelect::new().run()
            quote! { { compile_error!("TODO: select statement") } }
        }
    }
}

/// Transpile a parsed `GoSelect` AST into Rust code.
///
/// Generates a chain of `send_case()`, `recv_case()`, and `with_default()`
/// builder calls on `gourd::GoSelect`, followed by `.run()` to execute.
pub fn go_to_rust_select_ast(select: &GoSelect) -> TokenStream {
    let mut chain = quote! { gourd::GoSelect::<i32>::new() };

    for case in &select.cases {
        match case {
            GoSelectCase::Send { ch, value } => {
                chain = quote! { #chain.send_case(#ch, #value) };
            }
            GoSelectCase::Recv { ch, target: _ } => {
                chain = quote! {
                    #chain.recv_case(#ch, ::std::sync::Arc::new(::std::sync::Mutex::new(None::<i32>)))
                };
            }
            GoSelectCase::Default(block) => {
                if block.stmts.is_empty() {
                    chain = quote! { #chain.with_default() };
                } else {
                    let body_tokens = quote! { #block };
                    chain = quote! {
                        { #body_tokens; #chain.with_default() }
                    };
                }
            }
        }
    }

    quote! {
        {
            #chain.run();
        }
    }
}
