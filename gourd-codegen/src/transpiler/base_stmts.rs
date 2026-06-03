//! Base statement parser — the fallback for common statements in blocks.
//! Handles: `let` declarations, Go short declarations (`id := expr`),
//! standard expressions, and `make(...)` calls.

pub(crate) use super::ast::GoStmt;
use super::expr::go_to_rust;
use super::types::map_go_type_str;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::discouraged::Speculative;
use syn::parse_quote;
use syn::token;
use syn::{Expr, Stmt};

/// Parse a single base statement from a block.
/// Falls back to expression parsing and skips unmappable tokens.
pub(crate) fn parse_base_stmt(input: syn::parse::ParseStream, stmts: &mut Vec<GoStmt>) -> syn::Result<()> {
    // 1. Try `let` local declarations
    let fork = input.fork();
    if fork.peek(syn::token::Let) {
        match fork.parse::<Stmt>() {
            Ok(Stmt::Local(_)) => {
                if let Ok(Stmt::Local(local)) = input.parse() {
                    stmts.push(GoStmt::Local(local));
                    if input.peek(token::Semi) {
                        let _semi: token::Semi = input.parse()?;
                    }
                    return Ok(());
                }
            }
            _ => {}
        }

        // Handle `let m = map[K]V{entries}` when syn can't parse
        let let_fork = input.fork();
        if let_fork.parse::<syn::token::Let>().is_ok()
            && let_fork.parse::<syn::Ident>().is_ok()
            && let_fork.parse::<syn::token::Eq>().is_ok()
        {
            if let_fork.peek(syn::Ident) {
                let map_fork = let_fork.fork();
                if let Ok(kw) = map_fork.parse::<syn::Ident>() {
                    if kw == "map" && map_fork.peek(syn::token::Bracket) {
                        input.parse::<syn::token::Let>()?;
                        let ident = input.parse::<syn::Ident>()?;
                        let ident_str = ident.to_string();
                        input.parse::<syn::token::Eq>()?;
                        return super::slice_map::parse_go_map_decl(input, ident_str, stmts);
                    }
                }
            }
        }
    }

    // 2. Handle Go short variable declaration: `id := expr`
    let fork = input.fork();
    if fork.peek(syn::Ident) {
        let id_fork = fork.fork();
        if id_fork.parse::<syn::Ident>().is_ok()
            && id_fork.parse::<syn::token::Colon>().is_ok()
            && id_fork.peek(syn::token::Eq)
        {
            let ident = input.parse::<syn::Ident>()?;
            let _: syn::token::Colon = input.parse()?;
            let _: syn::token::Eq = input.parse()?;

            // Check if the value is a Go map literal
            let val_fork = input.fork();
            let map_fork = val_fork.fork();
            if let Ok(first_tt) = map_fork.parse::<proc_macro2::TokenTree>() {
                if let proc_macro2::TokenTree::Ident(map_kw) = &first_tt {
                    if *map_kw == "map" {
                        return super::slice_map::parse_go_map_decl(input, ident.to_string(), stmts);
                    }
                }
            }

            // Check for Go closure: `name := func(params) { body }`
            let cv_fork = input.fork();
            if let Ok(func_id) = cv_fork.parse::<syn::Ident>() {
                if func_id.to_string() == "func" {
                    // This is a Go closure! Parse the full assignment as `GoLocal(name, closure)`
                    // We need to skip `name := ` and then pass the closure to go_to_rust_closure
                    let _ = input.parse::<syn::token::Colon>();
                    let _ = input.parse::<syn::token::Eq>();
                    // Now `input` is at `func ...`
                    // Pass the rest to go_to_rust_closure
                    let closure_tokens: TokenStream = input.parse().unwrap_or_default();
                    let closure_expr = super::free_fn::go_to_rust_closure(closure_tokens);
                    stmts.push(GoStmt::GoLocal(ident, closure_expr));
                    if input.peek(token::Semi) {
                        let _semi: token::Semi = input.parse()?;
                    }
                    return Ok(());
                }
            }

            // Check for `make(...)` in short declarations
            let mval_fork = input.fork();
            let is_make = matches!(mval_fork.parse::<syn::Ident>(), Ok(ref id) if id.to_string() == "make")
                && mval_fork.peek(syn::token::Paren);
            if is_make {
                let _: syn::Ident = input.parse()?;
                let full_str = mval_fork.cursor().token_stream().to_string();
                let raw_args = extract_make_args(&full_str);
                let normalized = normalize_make_args(&raw_args);
                let make_expr = match_make(&normalized);
                stmts.push(GoStmt::GoLocal(ident, make_expr));
                if input.peek(token::Semi) {
                    let _semi: token::Semi = input.parse()?;
                }
                return Ok(());
            }

            let val: Expr = input.parse()?;
            let val_rust = go_to_rust(&val);
            stmts.push(GoStmt::GoLocal(ident, val_rust));
            if input.peek(token::Semi) {
                let _semi: token::Semi = input.parse()?;
            }
            return Ok(());
        }
    }

    // 3. Try standard expression parsing
    let fork = input.fork();
    if let Ok(expr) = fork.parse::<Expr>() {
        input.advance_to(&fork);
        stmts.push(GoStmt::Expr(expr));
        if input.peek(token::Semi) {
            let _semi: token::Semi = input.parse()?;
        }
        return Ok(());
    }

    // 4. Fallback: handle `make(...)` that syn can't parse
    let make_fork = input.fork();
    let is_make = matches!(make_fork.parse::<syn::Ident>(), Ok(ref id) if id.to_string() == "make")
        && make_fork.peek(syn::token::Paren);
    if is_make {
        input.parse::<syn::Ident>()?;
        let full_str = make_fork.cursor().token_stream().to_string();
        let raw_args = extract_make_args(&full_str);
        stmts.push(GoStmt::GoMake(raw_args));
        if input.peek(token::Semi) {
            let _semi: token::Semi = input.parse()?;
        }
        return Ok(());
    }

    // Nothing matched — skip one token tree to make progress
    let _ = input.parse::<proc_macro2::TokenTree>();
    Ok(())
}

/// Extract raw arguments from a `make(...)` call string.
fn extract_make_args(full_str: &str) -> String {
    if let Some(start) = full_str.find('(') {
        if let Some(end) = full_str.rfind(')') {
            full_str[start + 1..end].to_string()
        } else {
            String::new()
        }
    } else {
        String::new()
    }
}

/// Normalize token spacing in make arguments.
fn normalize_make_args(raw_args: &str) -> String {
    raw_args
        .replace(" [", "[")
        .replace(" ]", "]")
        .replace("  ", " ")
}

/// Match normalized make arguments to Rust code.
fn match_make(normalized: &str) -> TokenStream {
    match normalized {
        s if s.starts_with("chan ") => {
            let chan_args: Vec<&str> = s.splitn(2, ',').collect();
            let chan_type_str = chan_args[0].trim().trim_start_matches("chan ").trim();
            let chan_type = map_go_type_str(chan_type_str);
            if chan_args.len() == 2 {
                let cap_str = chan_args[1].trim();
                let cap: TokenStream = parse_quote! { #cap_str };
                quote! { GoChannel::<#chan_type>::with_capacity(#cap) }
            } else {
                quote! { GoChannel::<#chan_type>::new() }
            }
        }
        s if s.starts_with("map[") => {
            quote! { ::std::collections::HashMap::new() }
        }
        s if s.starts_with("[]") => {
            let slice_args: Vec<&str> = s.splitn(2, ',').collect();
            let slice_type_str = slice_args[0].trim().trim_start_matches("[]").trim();
            let slice_type = map_go_type_str(slice_type_str);
            if slice_args.len() == 2 {
                let len_str = slice_args[1].trim();
                let len: TokenStream = parse_quote! { #len_str };
                quote! { ::std::iter::repeat(#slice_type).take(#len).collect::<Vec::<#slice_type>>() }
            } else {
                quote! { ::std::iter::repeat(#slice_type).take(0).collect::<Vec::<#slice_type>>() }
            }
        }
        _ => {
            let msg = format!("TODO: make with unsupported type: {}", normalized.trim());
            quote! { { compile_error!(concat!("TODO: make with unsupported type: ", #msg)) } }
        }
    }
}
