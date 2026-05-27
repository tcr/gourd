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
        // Parse `[]` (optionally with a type inside like `[]int`), then `{elems}`
        // First, try to parse a bracket group (could be empty `[]` or `[]Type`)
        let bracket_content;
        let _ = syn::bracketed!(bracket_content in input);

        // Parse the element type (handles `[]` or `[]Type`)
        if !bracket_content.is_empty() {
            let _elem_type: syn::Type = bracket_content.parse()?;
        }

        // Now parse `{e1, e2, ...}`
        let brace_content;
        let _brace = syn::braced!(brace_content in input);

        let mut elems = Vec::new();
        if !brace_content.is_empty() {
            while !brace_content.is_empty() {
                let expr = syn::Expr::parse(&brace_content)?;
                elems.push(expr);
                // Consume optional comma
                if !brace_content.is_empty() && brace_content.peek(token::Comma) {
                    let _: token::Comma = brace_content.parse()?;
                } else {
                    break;
                }
            }
        }

        // If bracket_content was non-empty, store a dummy type for inference
        let elem_type = if !bracket_content.is_empty() { Some(syn::Type::Path(syn::TypePath {
            path: syn::Path::from(Ident::new("dummy_type_for_inference", proc_macro2::Span::call_site())),
            qself: None,
        })) } else { None };

        Ok(GoSliceLit { elem_type, elems })
    }
}

impl Parse for GoMapLit {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        //Parse `map` keyword
        let kw: syn::Ident = input.call(syn::Ident::parse_any)?;
        let kw_str = kw.to_string();
        if kw_str != "map" {
            return Err(input.error("expected `map` keyword"));
        }

        // Parse `[K]V` — bracketed key type, then value type
        let bracket_content;
        let _bracket = syn::bracketed!(bracket_content in input);
        let key_type: syn::Type = bracket_content.parse()?;

        // Value type follows (could be `int`, `string`, or another identifier)
        let val_type: syn::Type = input.parse()?;

        // Parse `{key: val, key2: val2, ...}`
        let brace_content;
        let _brace = syn::braced!(brace_content in input);

        let mut entries = Vec::new();
        if !brace_content.is_empty() {
            // Speculatively parse entries: key: value patterns
            while !brace_content.is_empty() {
                let (key, value) = parse_map_entry(&brace_content)?;
                entries.push((key, value));
                // Consume optional comma
                if !brace_content.is_empty() && brace_content.peek(token::Comma) {
                    let _comma: token::Comma = brace_content.parse()?;
                } else {
                    break;
                }
            }
        }

        Ok(GoMapLit { key_type: Some(key_type), val_type: Some(val_type), entries })
    }
}

/// Parse a map entry: key: value pair
pub(crate) fn parse_map_entry(input: ParseStream) -> syn::Result<(Expr, Expr)> {
    // Key could be: path identifier, or literal
    let key: Expr = input.parse()?;
    // Expect colon separator
    let _: syn::token::Colon = input.parse()?;
    // Parse value
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
/// This is called from the proc-macro entry point after checking the token shape.
pub fn parse_go_slice(tokens: &proc_macro2::TokenStream) -> syn::Result<GoSliceLit> {
    use proc_macro2::TokenTree;
    let mut iter = tokens.clone().into_iter();

    // First token must be a Group with Bracket delimiter (the `[]` part)
    match iter.next() {
        Some(TokenTree::Group(group)) if group.delimiter() == proc_macro2::Delimiter::Bracket => {
            // Group contains either empty `[]` or `[]int` etc.
            // Get the remaining tokens: `{elem1, elem2, ...}`
            let remaining: proc_macro2::TokenStream = iter.collect();

            // Construct a synthetic TokenStream that GoSliceLit::parse can work with:
             // Original: Group(Bracket, ...) + remaining (which starts with Group(Curly, ...))
            // We need to tell GoSliceLit's Parse impl: "first there's a bracket group, then a brace group"
             // Just rebuild: Group(Bracket, <contents>) + Group(Curly, <contents of brace group>)

            // The bracket group content is either empty or a type identifier
            let bracket_inner: proc_macro2::TokenStream = group.stream();

            // Now we need to extract the brace group content from `remaining`
            let brace_inner: proc_macro2::TokenStream = extract_brace_content(&remaining)?;

            // Reconstruct: [bracket] + {brace_content}
            let mut synthetic: proc_macro2::TokenStream = proc_macro2::TokenStream::new();

            // Bracket group — use the same content as the original (handles `[]` or `[]int`)
            synthetic.extend(Some(TokenTree::Group(proc_macro2::Group::new(
                proc_macro2::Delimiter::Bracket,
                bracket_inner,
            ))));
            // Brace group — use extracted content
            synthetic.extend(Some(TokenTree::Group(proc_macro2::Group::new(
                proc_macro2::Delimiter::Brace,
                brace_inner,
            ))));

            // Parse as GoSliceLit
            syn::parse2::<GoSliceLit>(synthetic).map_err(|e| {
                syn::Error::new(e.span(), format!("expected Go slice literal: {}", e))
            })
        }
        _ => Err(syn::Error::new(proc_macro2::Span::call_site(), "expected Go slice literal starting with `[]` or `[Type]`")),
    }
}

/// Top-level parse function for Go map literals: `map[K]V{key: val, ...}`
/// This is called from the proc-macro entry point after checking the token shape.
pub fn parse_go_map(tokens: &proc_macro2::TokenStream) -> syn::Result<GoMapLit> {
    use proc_macro2::TokenTree;

    // tokens already start with the `map` keyword (checked by lib.rs)
    let mut iter = tokens.clone().into_iter();

    // Skip the `map` ident
    match iter.next() {
        Some(TokenTree::Ident(id)) => {
            if id != "map" {
                return Err(syn::Error::new(proc_macro2::Span::call_site(), "expected `map` keyword"));
            }
        }
        _ => return Err(syn::Error::new(proc_macro2::Span::call_site(), "expected `map` keyword")),
    }

    // Next comes `[K]` — a bracket group
    let group_tree: TokenTree = iter.next()
        .ok_or_else(|| syn::Error::new(proc_macro2::Span::call_site(), "expected `[K]` after `map`"))?;

    match group_tree {
        TokenTree::Group(group) if group.delimiter() == proc_macro2::Delimiter::Bracket => {
            let bracket_inner: proc_macro2::TokenStream = group.stream();

            // Remaining tokens: value type + `{entries}` (brace group)
            let remaining: proc_macro2::TokenStream = iter.collect();

            // Extract the brace content from `remaining`
            let brace_inner: proc_macro2::TokenStream = extract_brace_content(&remaining)?;

            // Parse the value type (everything before the brace group in `remaining`)
            let val_type: Option<syn::Type> = {
                let mut has_val_type = false;
                for tt in remaining.clone() {
                    if let TokenTree::Group(g) = &tt
                        && g.delimiter() == proc_macro2::Delimiter::Brace {
                        break;
                    }
                    has_val_type = true;
                    // Collect type tokens (identifiers)
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

            // Construct a synthetic TokenStream that GoMapLit::parse can work with:
            // map ident + [bracket_key] valtype {entries}
            let mut synthetic: proc_macro2::TokenStream = proc_macro2::TokenStream::new();

            // Add the `map` keyword (already consumed and validated)
            synthetic.extend(Some(TokenTree::Ident(
                proc_macro2::Ident::new("map", proc_macro2::Span::call_site()),
            )));

            // Add the bracket group (key type)
            synthetic.extend(Some(TokenTree::Group(proc_macro2::Group::new(
                proc_macro2::Delimiter::Bracket,
                bracket_inner,
            ))));

            // Add the value type token(s) before the brace group
            // The val_type is either None (unnamed map) or the parsed type
            if let Some(vt) = val_type {
                // Insert val_type tokens after the bracket
                let val_stream: proc_macro2::TokenStream = quote! { #vt };
                synthetic.extend(val_stream);
            }

             // Add the brace group with entries
             synthetic.extend(Some(TokenTree::Group(proc_macro2::Group::new(
                 proc_macro2::Delimiter::Brace,
                 brace_inner,
             ))));

             // Parse as GoMapLit
             syn::parse2::<GoMapLit>(synthetic).map_err(|e| {
                 syn::Error::new(e.span(), format!("expected Go map literal: {}", e))
             })
         }
         _ => Err(syn::Error::new(proc_macro2::Span::call_site(), "expected `[K]` after `map`")),
     }
}

/// Extract the content (inner tokens) from the first Curly Group in a TokenStream.
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
