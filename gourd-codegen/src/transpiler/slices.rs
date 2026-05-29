//! Go slice and map literal parsing utilities.
#![allow(dead_code)]
use super::go_to_rust;
use proc_macro2::TokenStream;
use quote::quote;
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::token;
use syn::{Expr, Ident};

/// Go slice literal: `[]Type{elem1, elem2, ...}`
/// Parsed from Go source inside expressions, transpiles to Rust `vec![elem1, elem2, ...]`.
pub struct GoSliceLit {
    #[allow(dead_code)]
    pub(crate) elem_type: Option<syn::Type>,
    pub elems: Vec<Expr>,
}

/// Go map literal: `map[K]V{key1: val1, key2: val2, ...}`
/// Parsed from Go source, transpiles to Rust `std::collections::HashMap`.
pub struct GoMapLit {
    #[allow(dead_code)] pub key_type: Option<syn::Type>,
    #[allow(dead_code)] pub val_type: Option<syn::Type>,
    pub entries: Vec<(Expr, Expr)>,  // (key, value) pairs
}

impl Parse for GoSliceLit {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let bracket_content;
        let _ = syn::bracketed!(bracket_content in input);

        // Skip optional type inside `[]` — don't require it to be a valid
        // Rust type. Just try parsing as a type; if it fails, skip the
        // remaining tokens in the bracket content.
        if !bracket_content.is_empty() {
            if bracket_content.parse::<syn::Type>().is_err() {
                // Not a valid Rust type — consume as generic tokens
                let _: proc_macro2::TokenStream = bracket_content.parse()?;
            }
        }

        let brace_content;
        let _brace = syn::braced!(brace_content in input);

        let mut elems = Vec::new();
        if !brace_content.is_empty() {
            while !brace_content.is_empty() {
                let expr = syn::Expr::parse(&brace_content)?;
                elems.push(expr);
                if !brace_content.is_empty() && brace_content.peek(token::Comma) {
                    let _: token::Comma = brace_content.parse()?;
                } else {
                    break;
                }
            }
        }

        let elem_type = if !bracket_content.is_empty() { Some(syn::Type::Path(syn::TypePath {
            path: syn::Path::from(Ident::new("dummy_type_for_inference", proc_macro2::Span::call_site())),
            qself: None,
        })) } else { None };

        Ok(GoSliceLit { elem_type, elems })
    }
}

impl Parse for GoMapLit {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let kw: syn::Ident = input.call(syn::Ident::parse_any)?;
        let kw_str = kw.to_string();
        if kw_str != "map" {
            return Err(input.error("expected `map` keyword"));
        }

        let bracket_content;
        let _bracket = syn::bracketed!(bracket_content in input);
        // Skip key type — try parsing as Rust type, but don't fail
        let key_type: Option<syn::Type> = bracket_content.parse().ok();

        // Skip value type — try parsing as Rust type, but don't fail
        let val_type: Option<syn::Type> = input.parse().ok();

        let brace_content;
        let _brace = syn::braced!(brace_content in input);

        let mut entries = Vec::new();
        if !brace_content.is_empty() {
            while !brace_content.is_empty() {
                let (key, value) = parse_map_entry(&brace_content)?;
                entries.push((key, value));
                if !brace_content.is_empty() && brace_content.peek(token::Comma) {
                    let _comma: token::Comma = brace_content.parse()?;
                } else {
                    break;
                }
            }
        }

        Ok(GoMapLit { key_type, val_type, entries })
    }
}

pub(crate) fn parse_map_entry(input: ParseStream) -> syn::Result<(Expr, Expr)> {
    let key: Expr = input.parse()?;
    let _: syn::token::Colon = input.parse()?;
    let value: Expr = input.parse()?;
    Ok((key, value))
}

pub fn go_to_rust_slice(input: &GoSliceLit) -> TokenStream {
    let elems: Vec<_> = input.elems.iter().map(go_to_rust).collect();
    quote! { vec![ #(#elems),* ] }
}

pub fn go_to_rust_map(input: &GoMapLit) -> TokenStream {
    if input.entries.is_empty() {
        return quote! { std::collections::HashMap::new() };
    }

    let insertions: Vec<_> = input.entries.iter().map(|(k, v)| {
        let key = go_to_rust(k);
        let val = go_to_rust(v);
        quote! { m.insert(#key, #val); }
    }).collect();

    quote! { {
        let mut m = std::collections::HashMap::new();
        #(#insertions)*
        m
    } }
}

/// Top-level parse function for Go slice literals: `[]Type{...}` or `[]{...}`
pub fn parse_go_slice(tokens: &proc_macro2::TokenStream) -> syn::Result<GoSliceLit> {
    use proc_macro2::TokenTree;
    let mut iter = tokens.clone().into_iter();

    match iter.next() {
        Some(TokenTree::Group(group)) if group.delimiter() == proc_macro2::Delimiter::Bracket => {
            let remaining: proc_macro2::TokenStream = iter.collect();
            let bracket_inner: proc_macro2::TokenStream = group.stream();
            let brace_inner: proc_macro2::TokenStream = extract_brace_content(&remaining)?;

            let mut synthetic: proc_macro2::TokenStream = proc_macro2::TokenStream::new();
            synthetic.extend(Some(TokenTree::Group(proc_macro2::Group::new(
                proc_macro2::Delimiter::Bracket,
                bracket_inner,
            ))));
            synthetic.extend(Some(TokenTree::Group(proc_macro2::Group::new(
                proc_macro2::Delimiter::Brace,
                brace_inner,
            ))));

            syn::parse2::<GoSliceLit>(synthetic).map_err(|e| {
                syn::Error::new(e.span(), format!("expected Go slice literal: {}", e))
            })
        }
        _ => Err(syn::Error::new(proc_macro2::Span::call_site(), "expected Go slice literal starting with `[]` or `[Type]`")),
    }
}

/// Top-level parse function for Go map literals: `map[K]V{key: val, ...}`
pub fn parse_go_map(tokens: &proc_macro2::TokenStream) -> syn::Result<GoMapLit> {
    use proc_macro2::TokenTree;

    let mut iter = tokens.clone().into_iter();

    match iter.next() {
        Some(TokenTree::Ident(id)) => {
            if id != "map" {
                return Err(syn::Error::new(proc_macro2::Span::call_site(), "expected `map` keyword"));
            }
        }
        _ => return Err(syn::Error::new(proc_macro2::Span::call_site(), "expected `map` keyword")),
    }

    let group_tree: TokenTree = iter.next()
        .ok_or_else(|| syn::Error::new(proc_macro2::Span::call_site(), "expected `[K]` after `map`"))?;

    match group_tree {
        TokenTree::Group(group) if group.delimiter() == proc_macro2::Delimiter::Bracket => {
            let bracket_inner: proc_macro2::TokenStream = group.stream();
            let remaining: proc_macro2::TokenStream = iter.collect();
            let brace_inner: proc_macro2::TokenStream = extract_brace_content(&remaining)?;

            let val_type: Option<syn::Type> = {
                let mut has_val_type = false;
                for tt in remaining.clone() {
                    if let TokenTree::Group(g) = &tt
                        && g.delimiter() == proc_macro2::Delimiter::Brace {
                        break;
                    }
                    has_val_type = true;
                }
                if !has_val_type {
                    None
                } else {
                    let val_stream: proc_macro2::TokenStream = remaining
                        .clone()
                        .into_iter()
                        .take_while(|tt| {
                            if let proc_macro2::TokenTree::Group(g) = tt {
                                g.delimiter() != proc_macro2::Delimiter::Brace
                            } else {
                                true
                            }
                        })
                        .collect();
                    Some(syn::parse2::<syn::Type>(val_stream).map_err(|e| {
                        syn::Error::new(e.span(), format!("expected value type in map: {}", e))
                    })?)
                }
            };

            let mut synthetic: proc_macro2::TokenStream = proc_macro2::TokenStream::new();
            synthetic.extend(Some(TokenTree::Ident(
                proc_macro2::Ident::new("map", proc_macro2::Span::call_site()),
            )));
            synthetic.extend(Some(TokenTree::Group(proc_macro2::Group::new(
                proc_macro2::Delimiter::Bracket,
                bracket_inner,
            ))));
            if let Some(vt) = val_type {
                let val_stream: proc_macro2::TokenStream = quote! { #vt };
                synthetic.extend(val_stream);
            }
            synthetic.extend(Some(TokenTree::Group(proc_macro2::Group::new(
                proc_macro2::Delimiter::Brace,
                brace_inner,
            ))));

            syn::parse2::<GoMapLit>(synthetic).map_err(|e| {
                syn::Error::new(e.span(), format!("expected Go map literal: {}", e))
            })
        }
        _ => Err(syn::Error::new(proc_macro2::Span::call_site(), "expected `[K]` after `map`")),
    }
}

pub(crate) fn extract_brace_content(tokens: &proc_macro2::TokenStream) -> syn::Result<proc_macro2::TokenStream> {
    use proc_macro2::TokenTree;

    for tt in tokens.clone() {
        if let TokenTree::Group(g) = tt
            && g.delimiter() == proc_macro2::Delimiter::Brace {
            return Ok(g.stream());
        }
    }
    Err(syn::Error::new(proc_macro2::Span::call_site(), "expected `{...}` braces"))
}
