//! Go parameter, output, function, struct, and interface parsing.

pub(crate) use super::ast::{GoFn, GoFnInputs, GoFnOutput, GoInterface, GoInterfaceMethod, GoParam, GoStruct, GoStructField};
use proc_macro2::TokenTree;
use syn::ext::IdentExt;
use syn::parse::{discouraged::Speculative, Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token;
use syn::{Ident, Token};
use super::types::map_go_type_str;

impl Parse for GoFnInputs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = Vec::new();
        while !input.is_empty() {
            let id: Ident = input.parse()?;
            let mut group_ids: Vec<Ident> = Vec::new();

            // Look ahead for grouped parameters: `a, b, c int`
            let fork = input.fork();
            // Adjust fork for variadic: skip `...` if present
            if fork.peek(syn::token::DotDotDot) {
                let _ = fork.parse::<syn::token::DotDotDot>();
            }
            let mut ty_from_ident: Option<Box<syn::Type>> = None;

            // Detect variadic parameter: `name ...T`
            // Note: `...` is a single token (Ellipsis/DotDotDot) in Rust's token stream
            let is_variadic = input.peek(syn::token::DotDotDot);
            if is_variadic {
                let _: syn::token::DotDotDot = input.parse()?;
            }

            // Detect grouped params: after parsing the first param (`a`), peek ahead
            // for `a, b, c int` pattern (multiple names sharing one type).
            // Use input (not fork) as the loop driver to avoid mismatches.
            while input.peek(token::Comma) {
                let peek_fork = input.fork();
                let _ = peek_fork.parse::<token::Comma>();
                if peek_fork.peek(Ident) {
                    let name = peek_fork.parse::<Ident>()?;
                    let name_str = name.to_string();
                    let known_go_type = matches!(name_str.as_str(),
                        "bool" | "string" | "int" | "int8" | "int16" | "int32" | "int64"
                        | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr"
                        | "byte" | "rune" | "float32" | "float64" | "error" | "chan"
                    );
                    if known_go_type {
                        // The type is peek_fork — advance input to consume it
                        input.advance_to(&peek_fork);
                        // Parse the type from the fork's current position
                        ty_from_ident = Some(Box::new(input.parse()?));
                        break;
                    }
                    input.parse::<token::Comma>()?;
                    let param_name: Ident = input.parse()?;
                    group_ids.push(param_name);
                } else {
                    break;
                }
            }

            let fork = input.fork();
            let is_slice_like = fork.peek(syn::token::Bracket);

            // Parse type in normal path if not already set by group loop
            if ty_from_ident.is_none() {
                if !is_slice_like && fork.peek(syn::Ident) {
                    ty_from_ident = Some(input.parse()?);
                } else if !is_slice_like && fork.peek(syn::token::Colon) {
                    let _colon: syn::token::Colon = input.parse()?;
                    ty_from_ident = Some(input.parse()?);
                }
            }

            if is_slice_like {
                let content;
                let _ = syn::bracketed!(content in input);
                let elem_path: syn::Path = if content.is_empty() {
                    input.parse()?
                } else {
                    content.parse()?
                };
                let elem_type = syn::Type::Path(syn::TypePath {
                    path: elem_path,
                    qself: None,
                });
                args.push(GoParam { id: id.clone(), ty: None, slice_elem: Some(elem_type.clone()), variadic: is_variadic });
                for param_id in group_ids {
                    args.push(GoParam { id: param_id, ty: None, slice_elem: Some(elem_type.clone()), variadic: is_variadic });
                }
            } else {
                let ty = if let Some(ty) = ty_from_ident.clone() {
                    if let syn::Type::Path(tp) = &*ty
                        && tp.path.segments.len() == 1
                        && tp.path.segments.first().unwrap().ident.to_string() == "chan"
                    {
                        // Parse element type and build `chan<T>`
                        if input.peek(syn::Ident) {
                            let elem_ty: syn::Type = input.parse()?;
                            let mut chan_path = syn::Path::from(syn::Ident::new("chan", proc_macro2::Span::call_site()));
                            chan_path.segments.clear();
                            chan_path.segments.push(syn::PathSegment {
                                ident: syn::Ident::new("chan", proc_macro2::Span::call_site()),
                                arguments: syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                                    colon2_token: Default::default(),
                                    lt_token: Token![<](proc_macro2::Span::call_site()),
                                    args: syn::punctuated::Punctuated::from_iter([syn::GenericArgument::Type(elem_ty)]),
                                    gt_token: Token![>](proc_macro2::Span::call_site()),
                                }),
                            });
                            Some(Box::new(syn::Type::Path(syn::TypePath { path: chan_path, qself: None })))
                        } else {
                            Some(ty)
                        }
                    } else {
                        Some(ty)
                    }
                } else { None };
                let ty_for_param = ty.clone();
                args.push(GoParam { id: id.clone(), ty: ty_for_param, slice_elem: None, variadic: is_variadic });
                for param_id in group_ids {
                    args.push(GoParam { id: param_id, ty: ty.clone(), slice_elem: None, variadic: is_variadic });
                }
            }

            if input.peek(token::Comma) {
                input.parse::<token::Comma>()?;
            }
        }
        Ok(GoFnInputs { args })
    }
}

impl Parse for GoFnOutput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut tys: Vec<syn::Type> = Vec::new();
        let mut is_slice = false;
        let mut elem_type: Option<Box<syn::Type>> = None;
        if input.peek(syn::token::RArrow) {
            let _: syn::token::RArrow = input.parse()?;
        }
        if !input.peek(syn::token::Brace) {
            if input.peek(syn::token::Bracket) {
                is_slice = true;
                let content;
                let _bracket = syn::bracketed!(content in input);
                if input.peek(syn::token::Brace) {
                    tys.push(syn::Type::Path(syn::TypePath {
                        path: syn::Path::from(syn::Ident::new("__go_slice__", proc_macro2::Span::call_site())),
                        qself: None,
                    }));
                } else {
                    let elem = input.parse::<syn::Type>()?;
                    elem_type = Some(Box::new(elem));
                    tys.push(syn::Type::Path(syn::TypePath {
                        path: syn::Path::from(syn::Ident::new("__go_slice__", proc_macro2::Span::call_site())),
                        qself: None,
                    }));
                }
            } else if input.peek(syn::Ident) {
                let fork = input.fork();
                if let Ok(first_ident) = fork.parse::<syn::Ident>() {
                    let first_name = first_ident.to_string();
                    if first_name == "chan" {
                        let _: syn::Ident = input.parse()?;
                        let elem = if input.peek(syn::token::Bracket) {
                            let content;
                            let _bracket = syn::bracketed!(content in input);
                            content.parse::<syn::Type>().unwrap_or_else(|_| {
                                syn::Type::Path(syn::TypePath {
                                    path: syn::Path::from(syn::Ident::new("i32", proc_macro2::Span::call_site())),
                                    qself: None,
                                })
                            })
                        } else if input.peek(syn::Ident) {
                            input.parse::<syn::Type>().unwrap_or_else(|_| {
                                syn::Type::Path(syn::TypePath {
                                    path: syn::Path::from(syn::Ident::new("i32", proc_macro2::Span::call_site())),
                                    qself: None,
                                })
                            })
                        } else {
                            syn::Type::Path(syn::TypePath {
                                path: syn::Path::from(syn::Ident::new("i32", proc_macro2::Span::call_site())),
                                qself: None,
                            })
                        };
                        let mut chan_path = syn::Path::from(syn::Ident::new("__go_chan", proc_macro2::Span::call_site()));
                        chan_path.segments.clear();
                        chan_path.segments.push(syn::PathSegment {
                            ident: syn::Ident::new("__go_chan", proc_macro2::Span::call_site()),
                            arguments: syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                                colon2_token: None,
                                lt_token: Token![<](proc_macro2::Span::call_site()),
                                args: syn::punctuated::Punctuated::from_iter([
                                    syn::GenericArgument::Type(elem)
                                ]),
                                gt_token: Token![>](proc_macro2::Span::call_site()),
                            }),
                        });
                        tys.push(syn::Type::Path(syn::TypePath {
                            path: chan_path,
                            qself: None,
                        }));
                    } else if first_name == "map" {
                        let _: syn::Ident = input.parse()?;
                        if input.peek(syn::token::Bracket) {
                            let k_content;
                            let _bracket = syn::bracketed!(k_content in input);
                            let key_type: syn::Type = k_content.parse().unwrap_or_else(|_| {
                                syn::Type::Path(syn::TypePath {
                                    path: syn::Path::from(syn::Ident::new("string", proc_macro2::Span::call_site())),
                                    qself: None,
                                })
                            });
                            let val_type: syn::Type = if input.peek(syn::Ident) {
                                input.parse().unwrap_or_else(|_| {
                                    syn::Type::Path(syn::TypePath {
                                        path: syn::Path::from(syn::Ident::new("int", proc_macro2::Span::call_site())),
                                        qself: None,
                                    })
                                })
                            } else {
                                syn::Type::Path(syn::TypePath {
                                    path: syn::Path::from(syn::Ident::new("int", proc_macro2::Span::call_site())),
                                    qself: None,
                                })
                            };
                            let mut map_path = syn::Path::from(syn::Ident::new("__go_map", proc_macro2::Span::call_site()));
                            map_path.segments.clear();
                            map_path.segments.push(syn::PathSegment {
                                ident: syn::Ident::new("__go_map", proc_macro2::Span::call_site()),
                                arguments: syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                                    colon2_token: None,
                                    lt_token: Token![<](proc_macro2::Span::call_site()),
                                    args: syn::punctuated::Punctuated::from_iter([
                                        syn::GenericArgument::Type(key_type),
                                        syn::GenericArgument::Type(val_type),
                                    ]),
                                    gt_token: Token![>](proc_macro2::Span::call_site()),
                                }),
                            });
                            tys.push(syn::Type::Path(syn::TypePath {
                                path: map_path,
                                qself: None,
                            }));
                        }
                    } else {
                        let t = input.parse::<syn::Type>()?;
                        tys.push(t);
                    }
                } else {
                    let t = input.parse::<syn::Type>()?;
                    tys.push(t);
                }
            } else {
                let t = input.parse()?;
                tys.push(t);
            }
            while input.peek(token::Comma) {
                let _ = input.parse::<token::Comma>()?;
                if input.peek(syn::token::Brace) {
                    break;
                }
                if input.peek(syn::token::Bracket) {
                    is_slice = true;
                    let content;
                    let _bracket = syn::bracketed!(content in input);
                    if input.peek(syn::token::Brace) {
                        tys.push(syn::Type::Path(syn::TypePath {
                            path: syn::Path::from(syn::Ident::new("__go_slice__", proc_macro2::Span::call_site())),
                            qself: None,
                        }));
                    } else {
                        let elem = input.parse::<syn::Type>()?;
                        elem_type = Some(Box::new(elem));
                        tys.push(syn::Type::Path(syn::TypePath {
                            path: syn::Path::from(syn::Ident::new("__go_slice__", proc_macro2::Span::call_site())),
                            qself: None,
                        }));
                    }
                } else {
                    let t = input.parse()?;
                    tys.push(t);
                }
            }
        }
        Ok(GoFnOutput { tys, is_slice, elem_type })
    }
}

impl Parse for GoFn {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _fn: Ident = input.call(Ident::parse_any)?;
        let ident: Ident = input.parse()?;
        let generics = Punctuated::<syn::GenericParam, token::Comma>::new();
        if input.peek(syn::token::Bracket) {
            let content;
            let _bracketed = syn::bracketed!(content in input);
            Punctuated::<syn::GenericParam, token::Comma>::parse_terminated(&content)?;
        }
        let paren_content;
        let _paren = syn::parenthesized!(paren_content in input);
        let inputs = paren_content.parse()?;
        let output = if !input.is_empty() {
            let outer = input.parse()?;
            Some(outer)
        } else {
            None
        };
        let block = super::stmts::parse_go_block(input)?;
        Ok(GoFn { ident, generics, inputs, output, block })
    }
}

impl Parse for GoStruct {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _struct: Ident = input.call(Ident::parse_any)?;
        let ident: Ident = input.parse()?;

        // The struct body may be a brace-group (from Go source tokenization)
        // or an actual brace punctuation (from Rust tokenization).
        // Handle both cases.
        let mut fields = Vec::new();
        if input.peek(syn::token::Brace) {
            // Actual brace punctuation
            let content;
            syn::braced!(content in input);
            while !content.is_empty() {
                let name: Ident = content.parse()?;
                let ty: syn::Type = content.parse()?;
                fields.push(GoStructField { name, ty });
                loop {
                    let f = content.fork();
                    match f.parse::<proc_macro2::TokenTree>() {
                        Ok(TokenTree::Punct(p)) if p.as_char() == ',' => {
                            let _comma: token::Comma = content.parse()?;
                            break;
                        }
                        Ok(TokenTree::Punct(_)) => {
                            content.parse::<proc_macro2::TokenTree>()?;
                        }
                        Ok(_) => break,
                        Err(_) => break,
                    }
                }
            }
        } else {
            // The cursor is already inside the brace group content.
            // Use `step` to get the TokenStream from inside.
            let step_result: Result<proc_macro2::TokenStream, _> =
                input.step(|cursor| {
                    Ok((cursor.token_stream(), *cursor))
                });
            if let Ok(fork_ts) = step_result {
                let trees: Vec<proc_macro2::TokenTree> = fork_ts.into_iter().collect();
                let mut i = 0;
                while i < trees.len() - 1 {
                    if let proc_macro2::TokenTree::Ident(name_id) = &trees[i] {
                        let name = Ident::new(&name_id.to_string(), name_id.span());
                        if let proc_macro2::TokenTree::Ident(ty_id) = &trees[i + 1] {
                            let type_name = ty_id.to_string();
                            let ty = map_go_type_str(&type_name);
                            fields.push(GoStructField { name, ty });
                            i += 2;
                            continue;
                        }
                    }
                    i += 1;
                }
            }
        }

        Ok(GoStruct { ident, fields })
    }
}

impl Parse for GoInterface {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _interface_kw: Ident = input.call(Ident::parse_any)?;
        let ident: Ident = input.parse()?;
        let content;
        let _brace = syn::braced!(content in input);

        let mut methods = Vec::new();
        while !content.is_empty() {
            let method_fork = content.fork();
            if let Ok(name) = method_fork.parse::<Ident>() {
                let name_str = name.to_string();
                if matches!(name_str.as_str(),
                    "bool" | "string" | "int" | "int8" | "int16" | "int32" | "int64"
                    | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr"
                    | "byte" | "rune" | "float32" | "float64" | "error")
                {
                    break;
                }
                if matches!(name_str.as_str(),
                    "if" | "else" | "for" | "return" | "switch" | "case" | "default"
                    | "type" | "struct" | "func" | "interface" | "package" | "import" | "const" | "var")
                {
                    break;
                }

                content.parse::<Ident>()?;

                let param_paren;
                let _paren = syn::parenthesized!(param_paren in content);
                let inputs: GoFnInputs = param_paren.parse()?;

                let output = if !content.is_empty() {
                    let out_fork = content.fork();
                    if out_fork.peek(syn::token::RArrow) || out_fork.peek(Ident) || out_fork.peek(syn::token::Bracket) {
                        Some(content.parse::<GoFnOutput>()?)
                    } else {
                        None
                    }
                } else {
                    None
                };

                methods.push(GoInterfaceMethod { name, inputs, output });
            } else {
                break;
            }
        }

        Ok(GoInterface { ident, methods })
    }
}
