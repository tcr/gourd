//! Anonymous function (closure) transpilation.
//!
//! Converts Go anonymous functions (`func(params) ret { body }`) to
//! Rust closures (`|params| -> ret { body }`).

use proc_macro2::{TokenStream, TokenTree};
use quote::quote;
use syn::parse2;
use super::super::ast::GoBlock;
use super::super::stmt_to_rust::go_stmt_to_rust;

/// Top-level: parse and transpile a Go anonymous function to Rust closure.
pub fn go_to_rust_closure(input: TokenStream) -> TokenStream {
    let trees: Vec<TokenTree> = input.clone().into_iter().collect();
    eprintln!("DEBUG go_to_rust_closure: {}", input);

    // Validate: must start with `func`
    if trees.is_empty() {
        return quote! { { compile_error!("empty input"); } };
    }
    if let TokenTree::Ident(id) = &trees[0] {
        let name = id.to_string();
        if name != "func" && name != "fn" {
            return quote! { { compile_error!("not a function"); } };
        }
    } else {
        return quote! { { compile_error!("not a function"); } };
    }

    // Parse parameters from paren group
    let params: Vec<(proc_macro2::Ident, TokenStream)> = if trees.len() > 1 {
        if let TokenTree::Group(g) = &trees[1] {
            if g.delimiter() == proc_macro2::Delimiter::Parenthesis {
                let param_trees: Vec<TokenTree> = g.stream().into_iter().collect();
                parse_closure_params(&param_trees)
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // Parse optional return type
    let ret_type = if trees.len() > 2 {
        let next = &trees[2];
        match next {
            TokenTree::Ident(id) => {
                let type_name = id.to_string();
                if is_go_type_name(&type_name) {
                    Some(map_go_type_str(&type_name))
                } else {
                    None
                }
            }
            TokenTree::Group(g) => {
                let type_tokens: TokenStream = g.stream();
                syn::parse2::<syn::Type>(type_tokens).ok()
            }
            _ => None,
        }
    } else {
        None
    };

    // Parse body from brace group using GoBlock parsing
    let body_idx = if ret_type.is_some() { 3 } else { 2 };
    let body = if trees.len() > body_idx {
        if let TokenTree::Group(g) = &trees[body_idx] {
            if g.delimiter() == proc_macro2::Delimiter::Brace {
                let body_tokens: TokenStream = g.stream();
                // Parse the body as a GoBlock and transpile each statement
                // body_tokens is the content INSIDE braces (e.g., `return 0`)
                // GoBlock::parse expects brace-delimited content, so wrap in a brace group
                let wrapped_body: TokenStream = {
                    let brace_group = proc_macro2::Group::new(
                        proc_macro2::Delimiter::Brace,
                        body_tokens.clone(),
                    );
                    quote! { #brace_group }
                };
                eprintln!("DEBUG closure body: {}", wrapped_body);
                match parse2::<GoBlock>(wrapped_body) {
                    Ok(go_block) => {
                        let stmts: Vec<TokenStream> = go_block.stmts.iter().map(|s| go_stmt_to_rust(s)).collect();
                        eprintln!("DEBUG closure body parsed: {}", quote! { #(#stmts);* });
                        quote! { { #(#stmts);* } }
                    } Err(_) => {
                        // For bodies starting with `if`, handle them specially
                        let first_token = body_tokens.clone().into_iter().next();
                        if let Some(TokenTree::Ident(id)) = first_token {
                            if id.to_string() == "if" {
                                // Attempt to parse as Rust `if` expression
                                // The Go body is: `if cond { body } else { ... } body_continuation`
                                // We need to parse the if block and the remaining body
                                let if_result = syn::parse2::<syn::ExprIf>(body_tokens.clone());
                                if let Ok(if_expr) = if_result {
                                    let cond = if_expr.cond;
                                    let then_block: syn::Block = if_expr.then_branch;
                                    let else_block = if_expr.else_branch.as_ref().map(|(_, else_expr)| {
                                        let block: Box<syn::Block> = match else_expr.as_ref() {
                                            syn::Expr::Block(b) => Box::new(b.block.clone()),
                                            _ => syn::parse_quote!({}),
                                        };
                                        quote! { else #block }
                                    });
                                    quote! { { if #cond #then_block #else_block } }
                                } else {
                                    // Fallback: just pass through the tokens
                                    quote! { { #body_tokens } }
                                }
                            } else {
                                quote! { { #body_tokens } }
                            }
                        } else {
                            quote! { { compile_error!("could not parse closure body"); } }
                        }
                    }
                }
            } else {
                quote! { { compile_error!("expected body block"); } }
            }
        } else {
            quote! { { compile_error!("expected body block"); } }
        }
    } else {
        quote! { { compile_error!("no body"); } }
    };

    // Build Rust closure
    let rust_params: Vec<TokenStream> = params.iter().map(|p| {
        let id = &p.0;
        let ty: &TokenStream = &p.1;
        // ty is already a TokenStream from map_go_type_str
        // Just use it directly
        quote! { #id: #ty }
    }).collect();

    let ret = ret_type.as_ref().map(|ty| quote! { -> #ty });

    quote! { | #(#rust_params),* | #ret #body }
}

/// Parse closure parameters: `a int, b int` or `a, b int`.
fn parse_closure_params(trees: &[TokenTree]) -> Vec<(proc_macro2::Ident, TokenStream)> {
    let mut params = Vec::new();
    let mut i = 0;

    while i < trees.len() {
        // Skip commas
        if let TokenTree::Punct(p) = &trees[i] {
            if p.as_char() == ',' {
                i += 1;
                continue;
            }
        }

        // Parse identifier
        if let TokenTree::Ident(id) = &trees[i] {
            i += 1;

            // Check if next token is a type
            if i < trees.len() {
                let next = &trees[i];
                if let TokenTree::Ident(type_id) = next {
                    let type_name = type_id.to_string();
                    if is_go_type_name(&type_name) {
                        let ty = map_go_type_str(&type_name);
                        let ty: TokenStream = quote! { #ty };
                        params.push((id.clone(), ty));
                        i += 1;
                        continue;
                    }
                }
                // Slice type `[]T` - the `[]` is a bracket group (empty brackets)
                // followed by element type
                if let TokenTree::Group(g) = next {
                    if g.delimiter() == proc_macro2::Delimiter::Bracket {
                        // `[]` is empty bracket group, next token should be element type
                        if i + 1 < trees.len() {
                            if let TokenTree::Ident(elem_id) = &trees[i + 1] {
                                let elem_name = elem_id.to_string();
                                if is_go_type_name(&elem_name) {
                                    let rust_ty = map_go_type_str(&elem_name);
                                    // Convert []T to &[T] (Go slice -> Rust slice reference)
                                    let slice_ty: TokenStream = quote! { &[ #rust_ty ] };
                                    params.push((id.clone(), slice_ty));
                                    i += 2; // skip bracket group and element type
                                    continue;
                                }
                            }
                        }
                    }
                }

                // No type annotation - param is just an identifier, use unknown type
                params.push((id.clone(), quote! { unknown }));
                i += 1;
            }
        }

        // Skip unknown tokens to advance
        i += 1;
    }

    params
}

/// Check if a string is a Go type name.
fn is_go_type_name(name: &str) -> bool {
    matches!(name,
        "int" | "int8" | "int16" | "int32" | "int64"
        | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr"
        | "byte" | "rune"
        | "float32" | "float64"
        | "string" | "bool" | "error"
    )
}

/// Map a Go type string to Rust type.
fn map_go_type_str(go_type: &str) -> syn::Type {
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
fn map_go_types(ty: &syn::Type) -> syn::Type {
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
                            let mapped = map_go_types(elem_ty);
                            return syn::Type::Path(syn::TypePath {
                                qself: None,
                                path: syn::Path::from(syn::Ident::new("GoChannel", proc_macro2::Span::call_site())),
                            });
                        }
                    }
                    return map_go_type_str(&first_name);
                }
            }
            // Recursively map nested types
            let mut new_path = type_path.path.clone();
            for seg in new_path.segments.iter_mut() {
                if let syn::PathArguments::AngleBracketed(args) = &mut seg.arguments {
                    for arg in args.args.iter_mut() {
                        if let syn::GenericArgument::Type(ty) = arg {
                            *ty = map_go_types(ty);
                        }
                    }
                }
            }
            syn::Type::Path(syn::TypePath {
                qself: None,
                path: new_path,
            })
        }
        syn::Type::Slice(slice) => {
            // Go `[]T` → Rust `&[T]`
            let inner = map_go_types(&slice.elem);
            syn::Type::Reference(syn::TypeReference {
                and_token: Default::default(),
                lifetime: None,
                mutability: None,
                elem: Box::new(inner),
            })
        }
        _ => ty.clone(),
    }
}

/// Parse closure body statements.
fn parse_closure_body(body_tokens: &TokenStream) -> TokenStream {
    let trees: Vec<TokenTree> = body_tokens.clone().into_iter().collect();

    // Parse each statement
    let mut stmts = Vec::new();
    let mut i = 0;

    while i < trees.len() {
        let token = &trees[i];

        // Skip semicolons
        if let TokenTree::Punct(p) = token {
            if p.as_char() == ';' {
                i += 1;
                continue;
            }
        }

        // Try to parse as a `let` statement
        if let TokenTree::Ident(id) = token {
            if id.to_string() == "let" {
                i += 1; // skip 'let'

                // Parse pattern
                if i >= trees.len() {
                    break;
                }

                // Try to build let statement from tokens
                let mut let_tokens = Vec::new();
                let_tokens.push(token.clone()); // 'let'

                // Collect tokens until '='
                while i < trees.len() {
                    let_tokens.push(trees[i].clone());
                    if let TokenTree::Punct(p) = &trees[i] {
                        if p.as_char() == '=' {
                            i += 1;
                            break;
                        }
                    }
                    i += 1;
                }

                // Collect expression until ';' or end or next keyword
                while i < trees.len() {
                    if let TokenTree::Punct(p) = &trees[i] {
                        if p.as_char() == ';' {
                            i += 1;
                            break;
                        }
                        // Handle ',' in multi-assignment
                        if p.as_char() == ',' {
                            let_tokens.push(trees[i].clone());
                            i += 1;
                            continue;
                        }
                    }
                    let_tokens.push(trees[i].clone());
                    i += 1;
                }

                let let_ts: TokenStream = let_tokens.iter().cloned().collect();
                if let Ok(stmt) = syn::parse2::<syn::Expr>(let_ts.clone()) {
                    stmts.push(super::super::expr::dispatch::go_to_rust(&stmt));
                } else {
                    stmts.push(let_ts);
                }
                continue;
            }
        }

        // Try to parse as return statement
        if let TokenTree::Ident(id) = token {
            if id.to_string() == "return" {
                i += 1; // skip 'return'

                // Collect return value
                let mut ret_tokens = Vec::new();
                while i < trees.len() {
                    if let TokenTree::Punct(p) = &trees[i] {
                        if p.as_char() == ';' {
                            i += 1;
                            break;
                        }
                    }
                    ret_tokens.push(trees[i].clone());
                    i += 1;
                }

                let ret_ts: TokenStream = ret_tokens.iter().cloned().collect();
                if ret_ts.is_empty() {
                    stmts.push(quote! { return; });
                } else if let Ok(expr) = syn::parse2::<syn::Expr>(ret_ts.clone()) {
                    stmts.push(super::super::expr::dispatch::go_to_rust(&expr));
                } else {
                    stmts.push(ret_ts);
                }
                continue;
            }
        }

        // Try to parse as expression statement
        let mut expr_tokens = Vec::new();
        expr_tokens.push(trees[i].clone());
        i += 1;

        while i < trees.len() {
            if let TokenTree::Punct(p) = &trees[i] {
                if p.as_char() == ';' {
                    i += 1;
                    break;
                }
            }
            expr_tokens.push(trees[i].clone());
            i += 1;
        }

        let expr_ts: TokenStream = expr_tokens.iter().cloned().collect();
        if !expr_ts.is_empty() {
            if let Ok(expr) = syn::parse2::<syn::Expr>(expr_ts.clone()) {
                stmts.push(super::super::expr::dispatch::go_to_rust(&expr));
            } else {
                stmts.push(expr_ts);
            }
        }
    }

    quote! { { #(#stmts);* } }
}
