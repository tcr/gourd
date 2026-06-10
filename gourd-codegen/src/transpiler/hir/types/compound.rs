//! Interface parsing, type string mapping, and higher-level HIR types.
//!
//! Contains: parse_go_receiver_fn, map_go_type_str, map_go_types,
//! HirFunction, HirStruct, and their tests.

use crate::transpiler::hir::types::primitives::{HirType, HirTypeKind, HirInterfaceMethod, HirReceiverFn};
use crate::transpiler::hir::types::mapping::{go_type_to_hir, parse_go_type, parse_interface_params};
use crate::transpiler::hir::{HirStatement, HirBlock};
use crate::transpiler::hir::expression::{HirExpr, HirExprKind, HirLiteral};
use proc_macro2::{TokenStream, TokenTree, Delimiter};
use syn::Ident;


/// Parse a Go receiver function directly into HIR.
///
/// Input: `func (recv Type) name(params) output { body }`
/// Parsed into: HirReceiverFn with receiver, params, returns, and body.
pub fn parse_go_receiver_fn(input: TokenStream) -> Option<HirReceiverFn> {
    let trees: Vec<TokenTree> = input.into_iter().collect();
    if trees.len() < 4 {
        return None;
    }

    // tree[0] = "func"
    // tree[1] = receiver group (Parenthesis)
    // tree[2] = function name (Ident)
    // tree[3+] = parameters (Parenthesis), optional output, body (Brace)

    // Parse receiver from tree[1]
    let recv_group = match &trees[1] {
        TokenTree::Group(g) if g.delimiter() == Delimiter::Parenthesis => g.stream(),
        _ => return None,
    };
    let recv_trees: Vec<TokenTree> = recv_group.into_iter().collect();
    if recv_trees.len() < 2 {
        return None;
    }

    // Parse receiver name
    let recv_name = match &recv_trees[0] {
        TokenTree::Ident(id) => id.clone(),
        _ => return None,
    };

    // Parse receiver type (may have * prefix for pointer receivers)
    let (pointer, recv_type) = match &recv_trees[1] {
        TokenTree::Punct(p) if p.as_char() == '*' => {
            // Pointer receiver: skip the *
            if recv_trees.len() < 3 {
                return None;
            }
            match &recv_trees[2] {
                TokenTree::Ident(type_id) => {
                    (true, go_type_to_hir(&type_id.to_string()))
                }
                _ => return None,
            }
        }
        TokenTree::Ident(type_id) => {
            // Value receiver
            (false, go_type_to_hir(&type_id.to_string()))
        }
        _ => return None,
    };

    // Parse function name
    let fn_name = match &trees[2] {
        TokenTree::Ident(id) => id.clone(),
        _ => return None,
    };

    // Parse parameters from tree[3] if it's a paren group
    let mut params: Vec<(Ident, Box<HirType>)> = Vec::new();
    let mut remaining = if trees.len() > 3 {
        if let TokenTree::Group(g) = &trees[3] {
            if g.delimiter() == Delimiter::Parenthesis {
                params = parse_interface_params(g.stream());
            }
        }
        if trees.len() > 4 { 4 } else { return Some(HirReceiverFn {
            recv_name, recv_type, pointer, fn_name, params,
            returns: Vec::new(), body: None,
        }); }
    } else { return Some(HirReceiverFn {
        recv_name, recv_type, pointer, fn_name, params,
        returns: Vec::new(), body: None,
    }); };

    // Parse optional return type(s)
    let mut returns: Vec<Box<HirType>> = Vec::new();
    if remaining < trees.len() {
        if let TokenTree::Ident(ret_id) = &trees[remaining] {
            let ret_str = ret_id.to_string();
            returns.push(Box::new(parse_go_type(&ret_str)));
            remaining += 1;
        }
    }

    // Parse body from tree[remaining] if it's a brace group
    if remaining < trees.len() {
        if let TokenTree::Group(g) = &trees[remaining] {
            if g.delimiter() == Delimiter::Brace {
                let body_tokens: proc_macro2::TokenStream = g.stream();
                return Some(HirReceiverFn {
                    recv_name, recv_type, pointer, fn_name, params,
                    returns, body: Some(body_tokens),
                });
            }
        }
    }

    // No body found — return with what we have
    Some(HirReceiverFn {
        recv_name, recv_type, pointer, fn_name, params,
        returns, body: None,
    })
}

// ─── HIR types for select and switch statements ──────────────────────────────

/// HIR representation of a Go select statement.
pub struct HirSelect {
    pub cases: Vec<HirSelectCase>,
    pub default_body: Option<HirBlock>,
}

/// A single case in a Go select statement.
pub enum HirSelectCase {
    /// `case ch <- value:` — send case
    Send {
        ch: Box<HirExpr>,
        value: Box<HirExpr>,
    },
    /// `case <-ch:` — receive case  
    Recv {
        ch: Box<HirExpr>,
    },
    /// `default:` — default case
    Default,
}

/// HIR representation of a Go switch statement.
pub struct HirSwitch {
    pub selector: Option<Box<HirExpr>>,
    pub cases: Vec<HirSwitchCase>,
    pub default_body: Option<HirBlock>,
}

/// A single case in a Go switch statement.
pub struct HirSwitchCase {
    pub patterns: Vec<HirExpr>,
    pub body: HirBlock,
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::transpiler::hir::types::mapping::parse_go_interface;
    use quote::quote;

    #[test]
    fn test_parse_interface_basic() {
        let input = quote! { interface Shape { Name() string } };
        let result = parse_go_interface(input);
        assert!(result.is_some(), "Expected some result for basic interface");
    }

    #[test]
    fn test_parse_interface_empty() {
        let input = quote! { interface Empty {} };
        let result = parse_go_interface(input);
        // Empty interfaces are valid — should return Some with no methods
        assert!(result.is_some(), "Expected Some for empty interface");
        let ty = result.unwrap();
        match &ty.kind {
            HirTypeKind::Interface { name, methods } => {
                assert_eq!(name.to_string(), "Empty");
                assert!(methods.is_empty());
            }
            _ => panic!("Expected Interface kind"),
        }
    }

    #[test]
    fn test_parse_receiver_fn_value() {
        let input = quote! { func (s Foo) GetName() string { s.name } };
        let result = parse_go_receiver_fn(input);
        assert!(result.is_some(), "Expected Some for value receiver");
        let rf = result.unwrap();
        assert_eq!(rf.fn_name.to_string(), "GetName");
        assert!(!rf.pointer);
    }

    #[test]
    fn test_parse_receiver_fn_pointer() {
        let input = quote! { func (s *Foo) SetName(n string) { s.name = n } };
        let result = parse_go_receiver_fn(input);
        assert!(result.is_some(), "Expected Some for pointer receiver");
        let rf = result.unwrap();
        assert_eq!(rf.fn_name.to_string(), "SetName");
        assert!(rf.pointer);
    }
}

// ============================================================
// Legacy type mapping functions — moved from transpiler/types.rs
// These support parsing Go types during conversion.
// ============================================================

use syn::Token;

/// Map a Go type string (e.g., "int", "string", "rune") to a Rust type string.
/// Used for parsing `make()` call arguments where syn can't parse Go types.
pub(crate) fn map_go_type_str(go_type: &str) -> syn::Type {
    let rust_type = match go_type.trim() {
        "int" => "i32",
        "int8" => "i8",
        "int16" => "i16",
        "int32" => "i32",
        "int64" => "i64",
        "uint" => "u32",
        "uint8" => "u8",
        "uint16" => "u16",
        "uint32" => "u32",
        "uint64" => "u64",
        "uintptr" => "usize",
        "byte" => "u8",
        "rune" => "char",
        "float32" => "f32",
        "float64" => "f64",
        "string" => "String",
        "bool" => "bool",
        "error" => "Box<dyn std::error::Error>",
        _ => "unknown",
    };
    syn::parse_str::<syn::Type>(rust_type).unwrap_or_else(|_| {
        syn::Type::Path(syn::TypePath {
            path: syn::Path::from(syn::Ident::new(rust_type, proc_macro2::Span::call_site())),
            qself: None,
        })
    })
}

/// Map a single Go type identifier to its Rust equivalent.
/// Returns a `syn::Type` so that generic parameters can be recursed into.
pub(crate) fn map_go_types(ty: &syn::Type) -> syn::Type {
    match ty {
        syn::Type::Path(type_path) => {
            // Check for `__go_chan<T>` marker - converted to `GoChannel::<T>`
            if type_path.path.segments.len() == 1 {
                let first_name = type_path.path.segments.first().unwrap().ident.to_string();
                // Check for Go `chan T` syntax
                if first_name == "chan" {
                    // Extract element type from generic args: `chan<T>`
                    let seg = &type_path.path.segments[0];
                    if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                        if let Some(syn::GenericArgument::Type(elem_ty)) = args.args.first() {
                            // Map the element type first
                            let mapped_elem = map_go_types(elem_ty);
                            // Build GoChannel<T> with the mapped element type
                            let mut chan_path = syn::Path::from(syn::Ident::new("GoChannel", proc_macro2::Span::call_site()));
                            chan_path.segments.clear();
                            chan_path.segments.push(syn::PathSegment {
                                ident: syn::Ident::new("GoChannel", proc_macro2::Span::call_site()),
                                arguments: syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                                    colon2_token: None,
                                    lt_token: Token![<](proc_macro2::Span::call_site()),
                                    args: syn::punctuated::Punctuated::from_iter([
                                        syn::GenericArgument::Type(mapped_elem)
                                    ]),
                                    gt_token: Token![>](proc_macro2::Span::call_site()),
                                }),
                            });
                            return syn::Type::Path(syn::TypePath {
                                path: chan_path,
                                qself: None,
                            });
                        }
                    }
                }
                if first_name == "__go_chan" {
                    // Extract element type from generic args: `__go_chan<T>`
                    let seg = &type_path.path.segments[0];
                    if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                        if let Some(syn::GenericArgument::Type(elem_ty)) = args.args.first() {
                            // Map the element type first
                            let mapped_elem = map_go_types(elem_ty);
                            // Build GoChannel<T> with the mapped element type
                            let mut chan_path = syn::Path::from(syn::Ident::new("GoChannel", proc_macro2::Span::call_site()));
                            chan_path.segments.clear();
                            chan_path.segments.push(syn::PathSegment {
                                ident: syn::Ident::new("GoChannel", proc_macro2::Span::call_site()),
                                arguments: syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                                    colon2_token: None,
                                    lt_token: Token![<](proc_macro2::Span::call_site()),
                                    args: syn::punctuated::Punctuated::from_iter([
                                        syn::GenericArgument::Type(mapped_elem)
                                    ]),
                                    gt_token: Token![>](proc_macro2::Span::call_site()),
                                }),
                            });
                            return syn::Type::Path(syn::TypePath {
                                path: chan_path,
                                qself: None,
                            });
                        }
                    }
                }
                if first_name == "__go_map" {
                    // Extract key and value types from `__go_map<K, V>`
                    let seg = &type_path.path.segments[0];
                    if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                        let keys: Vec<_> = args.args.iter().collect();
                        if keys.len() >= 2 {
                            if let syn::GenericArgument::Type(key_ty) = &keys[0] {
                                if let syn::GenericArgument::Type(val_ty) = &keys[1] {
                                    let mapped_key = map_go_types(key_ty);
                                    let mapped_val = map_go_types(val_ty);
                                    // Build gourd::prelude::HashMap<K, V>
                                    let mut map_path = syn::Path::from(syn::Ident::new("gourd", proc_macro2::Span::call_site()));
                                    map_path.segments.push(syn::PathSegment::from(syn::Ident::new("prelude", proc_macro2::Span::call_site())));
                                    map_path.segments.push(syn::PathSegment {
                                        ident: syn::Ident::new("HashMap", proc_macro2::Span::call_site()),
                                        arguments: syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                                            colon2_token: None,
                                            lt_token: Token![<](proc_macro2::Span::call_site()),
                                            args: syn::punctuated::Punctuated::from_iter([
                                                syn::GenericArgument::Type(mapped_key),
                                                syn::GenericArgument::Type(mapped_val),
                                            ]),
                                            gt_token: Token![>](proc_macro2::Span::call_site()),
                                        }),
                                    });
                                    return syn::Type::Path(syn::TypePath {
                                        path: map_path,
                                        qself: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }

            // Check if the entire type is a single Go type identifier
            if type_path.path.segments.len() == 1 {
                let first_name = type_path.path.segments.first().unwrap().ident.to_string();
                if matches!(first_name.as_str(),
                    "bool" | "string" | "int" | "int8" | "int16" | "int32" | "int64"
                    | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr"
                    | "byte" | "rune" | "float32" | "float64" | "error"
                ) {
                    // Replace with the mapped Go type
                    let mapped_ident = match first_name.as_str() {
                        "bool" => "bool",
                        "string" => "String",
                        "int" => "i32",
                        "int8" => "i8",
                        "int16" => "i16",
                        "int32" => "i32",
                        "int64" => "i64",
                        "uint" => "u32",
                        "uint8" => "u8",
                        "uint16" => "u16",
                        "uint32" => "u32",
                        "uint64" => "u64",
                        "uintptr" => "usize",
                        "byte" => "u8",
                        "rune" => "char",
                        "float32" => "f32",
                        "float64" => "f64",
                        "error" => "Box<dyn std::error::Error>",
                        _ => unreachable!(),
                    };
                    return syn::Type::Path(syn::TypePath {
                        path: syn::Path::from(syn::Ident::new(mapped_ident, proc_macro2::Span::call_site())),
                        qself: None,
                    });
                }
            }

            // Handle generic types like `Vec<int>` by recursing into generic arguments
            let mut new_segments = type_path.path.segments.clone();
            for seg in new_segments.iter_mut() {
                if let syn::PathArguments::AngleBracketed(args) = &mut seg.arguments {
                    for arg in args.args.iter_mut() {
                        if let syn::GenericArgument::Type(ty) = arg {
                            *ty = map_go_types(&*ty);
                        }
                    }
                }
            }
            syn::Type::Path(syn::TypePath {
                path: syn::Path { segments: new_segments, leading_colon: type_path.path.leading_colon.clone() },
                qself: type_path.qself.clone(),
            })
        }
        syn::Type::Reference(type_ref) => {
            let elem = map_go_types(&type_ref.elem);
            match &type_ref.lifetime {
                Some(l) => syn::Type::Reference(syn::TypeReference {
                    and_token: type_ref.and_token,
                    lifetime: Some(l.clone()),
                    mutability: type_ref.mutability,
                    elem: Box::new(elem),
                }),
                None => syn::Type::Reference(syn::TypeReference {
                    and_token: type_ref.and_token,
                    lifetime: None,
                    mutability: type_ref.mutability,
                    elem: Box::new(elem),
                }),
            }
        }
        syn::Type::Slice(type_array) => {
            let elem = map_go_types(&type_array.elem);
            syn::Type::Slice(syn::TypeSlice {
                bracket_token: type_array.bracket_token,
                elem: Box::new(elem),
            })
        }
        syn::Type::Array(a) => {
            let elem = map_go_types(&a.elem);
            syn::Type::Array(syn::TypeArray {
                bracket_token: a.bracket_token,
                semi_token: a.semi_token,
                len: a.len.clone(),
                elem: Box::new(elem),
            })
        }
        syn::Type::Tuple(type_tuple) => {
            let elems: Vec<_> = type_tuple.elems.iter().map(|t| map_go_types(t)).collect();
            let paren_token = type_tuple.paren_token;
            syn::Type::Tuple(syn::TypeTuple {
                paren_token,
                elems: elems.into_iter().collect(),
            })
        }
        syn::Type::Paren(inner) => {
            let mapped = map_go_types(&inner.elem);
            syn::Type::Paren(syn::TypeParen {
                paren_token: inner.paren_token,
                elem: Box::new(mapped),
            })
        }
        _ => ty.clone(),
    }
}

// ─── HIR function and struct types ───────────────────────────────────────────

/// A higher-level function representation that captures the function's
/// semantic intent directly, avoiding token-level manipulation.
#[derive(Clone)]
pub struct HirFunction {
    /// Function name (preserved as camelCase from Go source)
    pub name: syn::Ident,
    /// Function parameters: (name, type) pairs
    pub params: Vec<(syn::Ident, Box<HirType>)>,
    /// Return types (empty if function returns nothing)
    pub returns: Vec<Box<HirType>>,
    /// Function body (block of statements)
    pub body: HirBlock,
}

/// A higher-level struct representation that captures the struct's
/// semantic intent directly, avoiding token-level manipulation.
#[derive(Clone)]
pub struct HirStruct {
    /// Struct name (preserved as camelCase from Go source)
    pub name: syn::Ident,
    /// Struct fields: (name, type) pairs
    pub fields: Vec<(syn::Ident, Box<HirType>)>,
}

impl HirFunction {
    /// Create a new empty HIR function.
    pub fn new(name: syn::Ident) -> Self {
        HirFunction {
            name,
            params: Vec::new(),
            returns: Vec::new(),
            body: HirBlock::new(),
        }
    }
}

impl HirStruct {
    /// Create a new empty HIR struct.
    pub fn new(name: syn::Ident) -> Self {
        HirStruct {
            name,
            fields: Vec::new(),
        }
    }
}

// ─── Helpers for parsing Go input into HIR types ─────────────────────────────
