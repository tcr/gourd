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
            // At this point, input is at `func ...` (after `:=` was consumed above)
            let cv_fork = input.fork();
            if let Ok(func_id) = cv_fork.parse::<syn::Ident>() {
                if func_id.to_string() == "func" {
                    // This is a Go closure! Parse it.
                    // Parse closure: func(params) { body } or func(params) ret { body }
                    // Consume func keyword
                    let _func: Option<syn::Ident> = input.parse().ok();
                    // Parse params group
                    let params_group: Option<proc_macro2::Group> = if input.peek(syn::token::Paren) {
                        input.parse::<proc_macro2::TokenTree>().ok().and_then(|tt| {
                            if let proc_macro2::TokenTree::Group(g) = tt {
                                Some(g)
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    };
                    // Parse optional return type
                    let ret_type: Option<syn::Ident> = if input.peek(syn::Ident) {
                        let ret_fork = input.fork();
                        if let Ok(ret_id) = ret_fork.parse::<syn::Ident>() {
                            let name = ret_id.to_string();
                            if matches!(name.as_str(), "int" | "int8" | "int16" | "int32" | "int64" | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr" | "byte" | "rune" | "float32" | "float64" | "string" | "bool" | "error") {
                                input.parse::<syn::Ident>().ok()
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    // Parse body brace group
                    let body_group: Option<proc_macro2::Group> = if input.peek(syn::token::Brace) {
                        input.parse::<proc_macro2::TokenTree>().ok().and_then(|tt| {
                            if let proc_macro2::TokenTree::Group(g) = tt {
                                Some(g)
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    };
                    // Reconstruct closure tokens
                    let closure_tokens: TokenStream = {
                        let mut ts = proc_macro2::TokenStream::new();
                        ts.extend([proc_macro2::TokenTree::Ident(syn::Ident::new("func", proc_macro2::Span::call_site()))]);
                        if let Some(g) = params_group {
                            ts.extend([proc_macro2::TokenTree::Group(g)]);
                        }
                        if let Some(ret_id) = ret_type {
                            ts.extend([proc_macro2::TokenTree::Ident(ret_id)]);
                        }
                        if let Some(g) = body_group {
                            ts.extend([proc_macro2::TokenTree::Group(g)]);
                        }
                        ts
                    };
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

            // Check for slice literal `[]T{...}` in short declarations
            if input.peek(syn::token::Bracket) {
                // This is a Go slice literal like `[]int{1, 2, 3}`
                // Extract elements from the brace-delimited group
                // Skip past `[]` and type
                let _ts: proc_macro2::TokenTree = input.parse()?; // [
                while !input.is_empty() && !input.peek(syn::token::Bracket) && !input.peek(syn::token::Brace) {
                    let _ = input.parse::<proc_macro2::TokenTree>()?;
                }
                if !input.is_empty() && input.peek(syn::token::Bracket) {
                    let _ts: proc_macro2::TokenTree = input.parse()?;
                }
                while !input.is_empty() && !input.peek(syn::token::Brace) {
                    let _ = input.parse::<proc_macro2::TokenTree>()?;
                }
                if input.peek(syn::token::Brace) {
                    let m_content;
                    let _ = syn::braced!(m_content in input);
                    let mut elems = Vec::new();
                    while !m_content.is_empty() {
                        if let Ok(expr) = m_content.parse::<Expr>() {
                            elems.push(expr);
                            if m_content.peek(syn::token::Comma) {
                                let _ = m_content.parse::<syn::token::Comma>();
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    let rust_elems: Vec<_> = elems.iter().map(|e| go_to_rust(e)).collect();
                    let slice_rust: TokenStream = parse_quote! { vec![ #(#rust_elems),* ] };
                    stmts.push(GoStmt::GoLocal(ident, slice_rust));
                    if input.peek(token::Semi) {
                        let _semi: token::Semi = input.parse()?;
                    }
                    return Ok(());
                }
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
    // Pre-process Go slice range syntax: `[start:end]` → `[start..end]`
    let fs = input.fork();
    if let Ok(expr) = fs.parse::<Expr>() {
        input.advance_to(&fs);
        stmts.push(GoStmt::Expr(expr));
        if input.peek(token::Semi) {
            let _semi: token::Semi = input.parse()?;
        }
        return Ok(());
    }

    // 3b. Try parsing with Go slice range preprocessing.
    // Go's `a[1:3]` is not valid Rust — convert to `a[1..3]` first.
    let expr_str = fs.cursor().token_stream().to_string();
    let converted = convert_go_slice_ranges(&expr_str);
    if let Ok(_expr) = syn::parse_str::<Expr>(&converted) {
        // Use the converted expression tokens directly
        let converted_ts: TokenStream = syn::parse_str::<Expr>(&converted)
            .map(|e| quote! { #e })
            .unwrap_or_default();
        stmts.push(GoStmt::RawStmt(converted_ts));
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
        let after_paren = &full_str[start + 1..];
        if let Some(end) = after_paren.find(')') {
            return after_paren[..end].to_string();
        }
    }
    String::new()
}

/// Normalize token spacing in make arguments.
fn normalize_make_args(raw_args: &str) -> String {
    raw_args
        .replace(" [", "[")
        .replace(" ]", "]")
        .replace("  ", " ")
}

/// Convert Go slice range syntax `[start:end]` to Rust range `[start..end]`.
/// Also handles `[start:]`, `[:end]`, and `[:]` (full slice).
pub(crate) fn convert_go_slice_ranges(expr_str: &str) -> String {
    let mut result = String::with_capacity(expr_str.len());
    let mut chars = expr_str.chars().peekable();
    let mut depth = 0usize;
    let mut in_bracket = false;

    while let Some(c) = chars.next() {
        match c {
            '[' => {
                depth += 1;
                in_bracket = depth >= 1;
                result.push(c);
            }
            ']' => {
                depth = depth.saturating_sub(1);
                in_bracket = depth >= 1;
                result.push(c);
            }
            ':' => {
                if in_bracket {
                    // Check if this colon is inside an index expression (not a type annotation)
                    // In Go `a[1:3]`, the colon is the slice range separator
                    // We need to convert it to Rust's `..`
                    // Look ahead to see if next non-space char is a digit or `]`
                    let mut peeked = Vec::new();
                    while let Some(&pc) = chars.peek() {
                        if pc == ' ' || pc == '\t' || pc == '\n' || pc == '\r' {
                            peeked.push(pc);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    if let Some(&nc) = chars.peek() {
                        if nc == ']' || nc.is_ascii_digit() || nc == '-' || nc.is_ascii_alphabetic() {
                            // This is a slice range colon — convert to `..`
                            result.push_str("..");
                            for pc in &peeked {
                                result.push(*pc);
                            }
                        } else {
                            // Not a slice range (e.g., type annotation inside brackets)
                            result.push(':');
                            for pc in &peeked {
                                result.push(*pc);
                            }
                        }
                    } else {
                        result.push(':');
                        for pc in &peeked {
                            result.push(*pc);
                        }
                    }
                } else {
                    result.push(c);
                }
            }
            _ => {
                result.push(c);
            }
        }
    }
    result
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
            // Extract key and value types from `map[K]V`
            if let Some(bracket_end) = s.find(']') {
                let key_str = s[4..bracket_end].trim();
                let val_str = s[bracket_end + 1..].trim();
                let key_type = map_go_type_str(key_str);
                let val_type = map_go_type_str(val_str);
                quote! { ::gourd::prelude::HashMap::<#key_type, #val_type>::default() }
            } else {
                quote! { ::gourd::prelude::HashMap::default() }
            }
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
