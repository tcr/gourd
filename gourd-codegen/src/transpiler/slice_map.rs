//! Map and slice literal parsing: map declarations, slice literals, element parsers.

use super::ast::{GoStmt};
use proc_macro2::TokenTree;
use syn::parse::discouraged::Speculative;
use syn::parse::{Parse, ParseStream};
use syn::token;
use syn::Expr;

/// Handle `id := map[K]V{entries}` map literal declaration.
pub(crate) fn parse_go_map_decl(input: ParseStream, ident_str: String, stmts: &mut Vec<GoStmt>) -> syn::Result<()> {
    let _kw: syn::Ident = input.parse()?; // consume 'map'

    let mut key_type: Option<Box<syn::Type>> = None;
    let bracket_fork = input.fork();
    if bracket_fork.peek(syn::token::Bracket) {
        input.advance_to(&bracket_fork);
        let _ts: TokenTree = input.parse()?;
        key_type = input.parse::<syn::Type>().ok().map(Box::new);
        if !input.is_empty() && input.peek(syn::token::Bracket) {
            let _ts: TokenTree = input.parse()?;
        }
    }

    let val_type = if !input.peek(syn::token::Brace) {
        input.parse::<syn::Type>().ok().map(Box::new)
    } else {
        None
    };

    let m_content = input.step(|cursor| {
        if let Some((inner, _, rest)) = cursor.group(proc_macro2::Delimiter::Brace) {
            Ok((inner.token_stream(), rest))
        } else {
            Err(cursor.error("expected `{`"))
        }
    });
    let mut entries = Vec::new();
    if let Ok(inner_ts) = m_content {
        if !inner_ts.is_empty() {
            let parser: MapEntryParser = syn::parse2(inner_ts).unwrap_or_default();
            entries = parser.entries;
        }
    }
    stmts.push(GoStmt::GoMap(ident_str, key_type, val_type, entries));
    if input.peek(token::Semi) {
        let _semi: token::Semi = input.parse()?;
    }
    Ok(())
}

/// Parse `[]T{...}` slice literal at the start of a statement.
pub(crate) fn parse_go_slice_literal(input: ParseStream, stmts: &mut Vec<GoStmt>) -> syn::Result<()> {
    let fork = input.fork();
    if fork.peek(syn::token::Bracket) {
        input.advance_to(&fork);
        let _ts: TokenTree = input.parse()?;

        while !input.is_empty() && !input.peek(syn::token::Bracket) && !input.peek(syn::token::Brace) {
            let _ = input.parse::<TokenTree>()?;
        }
        if !input.is_empty() && input.peek(syn::token::Bracket) {
            let _ts: TokenTree = input.parse()?;
        }
        while !input.is_empty() && !input.peek(syn::token::Brace) {
            let _ = input.parse::<TokenTree>()?;
        }

        if input.peek(syn::token::Brace) {
            let _ts: TokenTree = input.parse()?;
            let mut elems = Vec::new();
            while !input.is_empty() && !input.peek(syn::token::Brace) {
                let fork = input.fork();
                match fork.parse::<Expr>() {
                    Ok(expr) => {
                        input.advance_to(&fork);
                        elems.push(expr);
                        if input.peek(syn::token::Comma) {
                            let _ = input.parse::<syn::token::Comma>();
                        } else {
                            break;
                        }
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
            if input.peek(syn::token::Brace) {
                let _ts: TokenTree = input.parse()?;
            }
            stmts.push(GoStmt::GoSlice(elems));
            if input.peek(token::Semi) {
                let _semi: token::Semi = input.parse()?;
            }
            return Ok(());
        }
    }
    Err(syn::Error::new(proc_macro2::Span::call_site(), "expected slice literal"))
}

/// Parse comma-separated expressions from a group (e.g., slice elements).
#[derive(Default)]
pub(crate) struct ElemParser {
    pub(crate) elems: Vec<Expr>,
}

impl Parse for ElemParser {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut elems = Vec::new();
        while !input.is_empty() {
            let expr: Expr = input.parse()?;
            elems.push(expr);
            if input.peek(syn::token::Comma) {
                let _: syn::token::Comma = input.parse()?;
            } else {
                break;
            }
        }
        Ok(ElemParser { elems })
    }
}

/// Parse key-value pairs from a group (e.g., map literal entries).
#[derive(Default)]
pub(crate) struct MapEntryParser {
    pub(crate) entries: Vec<(Expr, Expr)>,
}

impl Parse for MapEntryParser {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut entries = Vec::new();
        while !input.is_empty() {
            // Parse key
            let key: Expr = input.parse()?;
            // Skip comma separator between keys (Go allows `key: val,` or `key: val`)
            if input.peek(syn::token::Comma) {
                let _ = input.parse::<syn::token::Comma>();
                // After comma, the next entry starts
                continue;
            }
            // Parse the `:` separator between key and value
            let _colon: syn::token::Colon = input.parse()?;
            // Parse value
            let value: Expr = input.parse()?;
            // Skip comma after value
            if input.peek(syn::token::Comma) {
                let _ = input.parse::<syn::token::Comma>();
            }
            entries.push((key, value));
        }
        Ok(MapEntryParser { entries })
    }
}
