//! Basic function and struct transpilation.
//!
//! Converts Go function declarations (`fn name() { ... }`) and struct
//! declarations (`struct Name { field type }`) into Rust.
//!
//! HIR-based transpilation is available via `go_to_rust_fn_hir()`.

use crate::transpiler::parsing::{GoFn, GoStruct};
use super::super::types::map_go_types;
use crate::transpiler::hir::{HirFunction, HirStatement, HirBlock, HirType, HirTypeKind, HirExpr, HirExprKind};
use crate::transpiler::hir::codegen::hir_stmt_to_rust;
use crate::transpiler::hir::conversion::{go_ast_expr_to_hir, go_stmt_to_hir, go_block_to_hir};
use crate::transpiler::hir::types::{parse_go_struct, parse_go_interface};
use proc_macro2::TokenStream;
use quote::quote;

/// Top-level: parse and transpile a Go function declaration to Rust.
/// Preprocess a token stream to convert Go slice range syntax `[start:end]`
/// to Rust slice range syntax `[start..end]`.
///
/// In CLI context, `[1:3]` is tokenized as a single `Group(Bracket)` token
/// containing the colon. We preprocess the group's content to replace `:`
/// with `..` so that `syn` can parse it as a Rust range expression.
fn preprocess_slice_ranges(ts: TokenStream) -> TokenStream {
    use proc_macro2::{TokenTree, Group, Delimiter, Punct, Spacing};

    /// Preprocess only **bracket** groups to convert colons to `..`.
    /// Brace groups (function bodies, struct bodies) are left untouched
    /// to avoid corrupting switch/case labels like `case 2:`.
    fn preprocess_bracket_groups(group: &Group) -> TokenStream {
        let tts: Vec<TokenTree> = group.stream().into_iter().collect();
        let mut result = Vec::new();

        for tt in tts {
            match tt {
                // Replace colons with `..` inside bracket groups only
                TokenTree::Punct(p) if p.as_char() == ':' => {
                    result.push(TokenTree::Punct(Punct::new('.', Spacing::Joint)));
                    result.push(TokenTree::Punct(Punct::new('.', Spacing::Alone)));
                }
                // Recursively preprocess nested groups — only if parent is bracket
                TokenTree::Group(inner_g)
                    if group.delimiter() == Delimiter::Bracket =>
                {
                    let inner_ts = preprocess_bracket_groups(&inner_g);
                    result.push(TokenTree::Group(Group::new(inner_g.delimiter(), inner_ts)));
                }
                _ => {
                    result.push(tt);
                }
            }
        }
        result.into_iter().collect()
    }

    ts.into_iter().map(|tt| {
        match tt {
            // Only preprocess bracket groups; leave brace groups alone
            TokenTree::Group(g)
                if g.delimiter() == Delimiter::Bracket =>
            {
                let inner = preprocess_bracket_groups(&g);
                TokenTree::Group(Group::new(Delimiter::Bracket, inner))
            }
            _ => tt,
        }
    }).collect()
}

pub fn go_to_rust_fn(input: TokenStream) -> TokenStream {
    let input = preprocess_slice_ranges(input);
    match syn::parse2::<GoFn>(input) {
        Ok(go_fn) => {
                // Preserve Go function name (camelCase stays camelCase)
            let fn_name = &go_fn.ident;
            let generics = &go_fn.generics;

            let output = go_fn.output.as_ref().map(|output| {
                if output.tys.is_empty() {
                    quote! {}
                } else {
                    let mapped: Vec<_> = output.tys.iter().map(|t| map_go_types(t)).collect();
                    match mapped.len() {
                        1 => {
                            let m = &mapped[0];
                            if output.is_slice {
                                // Use the stored element type for slices
                                if let Some(elem) = &output.elem_type {
                                    let mapped_elem = map_go_types(elem);
                                    quote! { -> Vec< #mapped_elem > }
                                } else {
                                    quote! { -> Vec< #m > }
                                }
                            } else {
                                quote! { -> #m }
                            }
                        }
                        _ => quote! { -> ( #(#mapped),* ) },
                    }
                }
            }).unwrap_or_else(|| quote! {});

            let mut all_params = Vec::<TokenStream>::new();
            for param in &go_fn.inputs.args {
                let id = &param.id;
                let variadic = param.variadic;
                match (&param.ty, &param.slice_elem) {
                    (None, None) => {
                        all_params.push(quote! { #id });
                    }
                    (_, Some(slice_inner)) => {
                        let mapped = map_go_types(slice_inner);
                        all_params.push(quote! { #id: &[ #mapped ]});
                    }
                    (Some(ty), None) => {
                        let mapped = map_go_types(ty);
                        if variadic {
                            // Variadic: `nums ...int` → `nums: &[i32]`
                            all_params.push(quote! { #id: &[ #mapped ] });
                        } else {
                            all_params.push(quote! { #id: #mapped });
                        }
                    }
                }
            }

            let mut stmts = Vec::new();
            for stm in &go_fn.block.stmts {
                stmts.push(crate::transpiler::parsing::go_stmt_to_rust(stm));
            }

            // If function has a return type and body is not empty,
            // wrap the last statement with `return` so it becomes the function's return value.
            let body: proc_macro2::TokenStream = if go_fn.output.is_some() && !stmts.is_empty() {
                let last = stmts.pop().unwrap();
                // Check if the last statement already starts with `return` keyword
                let last_str = last.to_string();
                let already_returns = last_str.trim_start().starts_with("return ") || last_str.trim_start() == "return";
                // Also check if it's a local declaration (let ...) — don't wrap with return
                let is_local = last_str.trim_start().starts_with("let ");
                if already_returns || is_local {
                    // Last statement already has `return` — just use it as-is
                    if stmts.is_empty() {
                        quote! { { #last } }
                    } else {
                        let all_but_last = &stmts;
                        quote! { { #(#all_but_last);*; #last } }
                    }
                } else {
                    // Last statement needs `return` wrapper
                    if stmts.is_empty() {
                        quote! { { return #last } }
                    } else {
                        let all_but_last = &stmts;
                        quote! { { #(#all_but_last);*; return #last } }
                    }
                }
            } else {
                quote!({ #(#stmts);* })
            };

            let body_str = body.to_string();
            eprintln!("DEBUG: FINAL body for {} = [{}]", go_fn.ident, body_str);

            let result = quote! {
                fn #fn_name #generics ( #(#all_params),* ) #output #body
            };
            result
        }
        Err(e) => {
            e.to_compile_error()
        }
    }
}

/// Top-level: parse and transpile a Go struct declaration to Rust.
pub fn go_to_rust_struct(input: TokenStream) -> TokenStream {
    match syn::parse2::<GoStruct>(input) {
        Ok(go_struct) => {
            let name = &go_struct.ident;
            let fields = go_struct.fields.iter().map(|f| {
                let fname = &f.name;
                let ftty = map_go_types(&f.ty);
                quote! { pub #fname: #ftty }
            });
            quote! {
                struct #name {
                    #(#fields),*
                }
            }
        }
        Err(e) => e.to_compile_error(),
    }
}

/// HIR-based struct transpilation.
///
/// Parses the Go struct declaration directly into HIR types, bypassing the Go AST.
pub fn go_to_rust_struct_hir(input: TokenStream) -> TokenStream {
    let hir_type = match parse_go_struct(input) {
        Some(ty) => ty,
        None => return quote! { compile_error!("Failed to parse Go struct") },
    };

    match &hir_type.kind {
        HirTypeKind::Struct { name, fields } => {
            crate::transpiler::hir::codegen::hir_struct_to_rust(name, fields)
        }
        _ => quote! { compile_error!("Expected struct type in HIR") },
    }
}

/// HIR-based function transpilation.
///
/// Converts a Go function to HIR, then generates Rust tokens.
/// This is the new pipeline that replaces the old token-level transpilation.
pub fn go_to_rust_fn_hir(input: TokenStream) -> TokenStream {
    // First parse the Go function into our custom AST
    let go_fn = match syn::parse2::<GoFn>(input) {
        Ok(go_fn) => go_fn,
        Err(e) => return e.to_compile_error(),
    };

    // Convert GoFn → HirFunction
    let hir_fn = go_fn_to_hir(&go_fn);

    // Generate Rust tokens from HIR
    hir_fn_to_rust(&hir_fn)
}

/// Convert a GoFn AST to a HirFunction.
fn go_fn_to_hir(go_fn: &GoFn) -> HirFunction {
    use crate::transpiler::hir::types::go_type_to_hir;
    use crate::transpiler::hir::types::parse_go_type;

    // Extract the function name
    let name = go_fn.ident.clone();

    // Convert parameters
    let params: Vec<(syn::Ident, Box<HirType>)> = go_fn.inputs.args.iter().map(|param| {
        let id = param.id.clone();
        let ty = match (&param.ty, &param.slice_elem) {
            (None, None) => {
                // Simple type with no slice element — fallback to i32
                Box::new(go_type_to_hir("int"))
            }
            (_, Some(slice_inner)) => {
                // Slice type: `[]T` → borrowed slice `&[T]` for parameters
                let elem = parse_go_type(&format!("[]{}", quote::quote! { #slice_inner }));
                // Unwrap the Slice variant and wrap as SliceRef for borrowed params
                match elem.kind {
                    HirTypeKind::Slice(inner) => {
                        Box::new(HirType::new(HirTypeKind::SliceRef(inner)))
                    }
                    _ => Box::new(elem),
                }
            }
            (Some(ty), None) => {
                // Regular type — use map_go_types for compound types (chan, map, etc.)
                let mapped = super::super::types::map_go_types(ty);
                let ty_str = quote::quote! { #mapped }.to_string();
                // Now parse the canonical Go→Rust type string
                Box::new(parse_go_type(&ty_str))
            }
        };
        (id, ty)
    }).collect();

    // Convert return types
    let returns: Vec<Box<HirType>> = go_fn.output.as_ref().map(|output| {
        if output.tys.is_empty() {
            Vec::new()
        } else {
            output.tys.iter().enumerate().map(|(i, t)| {
                // Handle slice return types: `[]T` → `Vec<T>`
                if output.is_slice && i == 0 {
                    if let Some(elem) = &output.elem_type {
                        Box::new(parse_go_type(&format!("[]{}", quote::quote! { #elem })))
                    } else {
                        Box::new(parse_go_type(&format!("[]{}", quote::quote! { #t })))
                    }
                } else {
                    let ty_str = quote::quote! { #t }.to_string();
                    // Use parse_go_type to handle compound types (maps, slices, etc.)
                    Box::new(parse_go_type(&ty_str))
                }
            }).collect()
        }
    }).unwrap_or_else(Vec::new);

    // Convert body statements using HIR conversion module
    // Note: go_fn.block.stmts is Vec<crate::transpiler::hir::ast::GoStmt>, which is compatible with go_stmt_to_hir.
    let body_stmts: Vec<HirStatement> = go_fn.block.stmts.iter()
        .map(|stm| {
            // Safe transmute: both GoStmt types have identical structure
            let hir_stmt: &crate::transpiler::hir::ast::GoStmt = unsafe { std::mem::transmute(stm) };
            go_stmt_to_hir(hir_stmt)
        })
        .collect();

    let body = HirBlock { stmts: body_stmts };

    HirFunction { name, params, returns, body }
}

/// Generate Rust tokens from a HirFunction.
fn hir_fn_to_rust(hir_fn: &HirFunction) -> TokenStream {
    let name = &hir_fn.name;

    // Generate parameter tokens
    let param_tokens: Vec<TokenStream> = hir_fn.params.iter().map(|(name, ty)| {
        let ty_tokens = ty.to_rust_type();
        quote! { #name: #ty_tokens }
    }).collect();

    // Generate return type tokens
    let return_tokens = if hir_fn.returns.is_empty() {
        quote! {}
    } else {
        let mapped: Vec<TokenStream> = hir_fn.returns.iter().map(|t| t.to_rust_type()).collect();
        if mapped.len() == 1 {
            quote! { -> #(#mapped)* }
        } else {
            quote! { -> ( #(#mapped),* ) }
        }
    };

    // Generate body tokens
    // Each statement already has its own separator/braces, just concatenate them.
    let body_tokens: Vec<TokenStream> = hir_fn.body.stmts.iter().map(|stmt| {
        hir_stmt_to_rust(stmt, true)
    }).collect();

    quote! {
        fn #name ( #(#param_tokens),* ) #return_tokens { #(#body_tokens)* }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transpiler::hir::types::map_go_types;
    use proc_macro2::TokenStream;
    use quote::quote;

    #[test]
    fn test_hir_fn_simple() {
        // Create a simple HIR function: `fn goHello() { 42 }`
        let name = syn::Ident::new("goHello", proc_macro2::Span::call_site());
        let stmt: syn::Expr = syn::parse_quote! { 42 };
        let body_stmt = HirStatement::Expr(Box::new(
            go_ast_expr_to_hir(&stmt)
        ));
        let body = HirBlock { stmts: vec![body_stmt] };
        let hir_fn = HirFunction {
            name,
            params: Vec::new(),
            returns: Vec::new(),
            body,
        };

        let tokens = hir_fn_to_rust(&hir_fn);
        let s = tokens.to_string();
        assert!(s.contains("goHello"), "Expected 'goHello' in output, got: {}", s);
        assert!(s.contains("42"), "Expected '42' in output, got: {}", s);
    }

    #[test]
    fn test_hir_fn_with_params() {
        // Create a simple HIR function with parameters: `fn goAdd(a: i32, b: i32) -> i32 { a + b }`
        use crate::transpiler::hir::types::{go_type_to_hir};

        let a_name = syn::Ident::new("a", proc_macro2::Span::call_site());
        let b_name = syn::Ident::new("b", proc_macro2::Span::call_site());
        let a_ty = map_go_types(&syn::parse_str("int").unwrap());
        let b_ty = map_go_types(&syn::parse_str("int").unwrap());
        let a_hir = go_type_to_hir("int");
        let b_hir = go_type_to_hir("int");

        let params = vec![
            (a_name, Box::new(a_hir)),
            (b_name, Box::new(b_hir)),
        ];

        let mut name = syn::Ident::new("goAdd", proc_macro2::Span::call_site());
        let return_hir = go_type_to_hir("int");

        let hir_fn = HirFunction {
            name,
            params,
            returns: vec![Box::new(return_hir)],
            body: HirBlock::new(),
        };

        let tokens = hir_fn_to_rust(&hir_fn);
        let s = tokens.to_string();
        assert!(s.contains("goAdd"), "Expected 'goAdd' in output, got: {}", s);
        assert!(s.contains("a"), "Expected 'a' in output, got: {}", s);
        assert!(s.contains("b"), "Expected 'b' in output, got: {}", s);
        assert!(s.contains("->"), "Expected '->' in output, got: {}", s);
    }
}
