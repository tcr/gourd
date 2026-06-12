//! Return statement parsing: single/multi-return, make(), append(), slice returns, type assertions.

use crate::transpiler::hir::ast::{GoStmt, Switch};
use proc_macro2::TokenStream;
use quote::quote;
use crate::transpiler::types::{go_to_rust_slice_arg, map_go_type_str, split_top_level_comma, split_top_level_items};
use syn::parse::discouraged::Speculative;
use syn::parse::ParseStream;
use syn::parse_quote;
use syn::token;
use syn::{Expr, Ident};


/// Parse `return` — handles single, multi-return, slice returns, make(), and append().
pub(crate) fn parse_go_return(input: ParseStream, stmts: &mut Vec<GoStmt>) -> syn::Result<bool> {
    input.parse::<syn::token::Return>()?;
    

    // Check for `return switch ...` - handle switch statement after return
    let switch_fork = input.fork();
    if switch_fork.peek(syn::Ident) {
        let kw_fork = switch_fork.fork();
        if let Ok(kw) = kw_fork.parse::<syn::Ident>() {
            let kw_str = kw.to_string();
            if kw_str == "switch" {
                
                // Reposition to switch_fork and parse switch from there
                input.advance_to(&switch_fork);
                // Now parse the switch statement (which includes the 'switch' keyword)
                let switch: Switch = input.parse()?;
                // Switch::ToTokens already produces transpiled Rust via HIR pipeline
                let switch_result: TokenStream = quote! { #switch };
                stmts.push(GoStmt::SwitchReturn(switch_result));
                if input.peek(token::Semi) {
                    let _semi: token::Semi = input.parse()?;
                }
                return Ok(true);
            }
        }
    }

    // Check for channel receive: `return <-ch`
    
    if input.cursor().punct().is_some() {
        if let Some((p, _)) = input.cursor().punct() {
            if p.as_char() == '<' && p.spacing() == proc_macro2::Spacing::Joint {
                
                let _p1: proc_macro2::Punct = input.parse()?;
                let _p2: proc_macro2::Punct = input.parse()?;
                let ch_ident: Ident = input.parse()?;
                let ch_expr = Expr::Path(syn::ExprPath { attrs: vec![], qself: None, path: syn::Path::from(ch_ident) });
                stmts.push(GoStmt::GoChannelRecv(ch_expr));
                if input.peek(token::Semi) {
                    let _semi: token::Semi = input.parse()?;
                }
                return Ok(true);
            }
        }
    }
    

    // Check for `return make(...)`
    let make_fork = input.fork();
    let saved_fork = input.fork();
    let make_ident = make_fork.parse::<syn::Ident>().ok().map(|id| id.to_string());
    let make_paren = make_fork.peek(syn::token::Paren);
    let is_make = make_ident.as_deref() == Some("make") && make_paren;
    if is_make {
        input.advance_to(&saved_fork);
        let _: syn::Ident = input.parse()?;
        let mut args_ts = TokenStream::new();
        let has_tt = input.cursor().token_tree();
        if let Some((proc_macro2::TokenTree::Group(group), _)) = has_tt {
            if group.delimiter() == proc_macro2::Delimiter::Parenthesis {
                args_ts.extend(group.stream());
            }
        }
        if input.peek(token::Semi) {
            let _semi: token::Semi = input.parse()?;
        }
        let raw_args = args_ts.to_string();
        let normalized = raw_args
            .replace(" [", "[")
            .replace(" ]", "]")
            .replace("  ", " ");
        let make_rust = match normalized.as_str() {
            s if s.starts_with("chan ") => {
                let chan_args: Vec<&str> = s.splitn(2, ',').collect();
                let chan_type_str = chan_args[0].trim().trim_start_matches("chan ").trim();
                let chan_type = map_go_type_str(chan_type_str);
                if chan_args.len() == 2 {
                    let cap_str = chan_args[1].trim();
                    let cap: syn::LitInt = syn::parse_str(cap_str).unwrap_or_else(
                        |_| syn::parse_quote!(0usize)
                    );
                    quote! { return GoChannel::<#chan_type>::with_capacity(#cap) }
                } else {
                    quote! { return GoChannel::<#chan_type>::new() }
                }
            }
            s if s.starts_with("map[") => {
                if let Some(bracket_end) = s.find(']') {
                    let key_str = s[4..bracket_end].trim();
                    let val_str = s[bracket_end + 1..].trim();
                    let key_type = map_go_type_str(key_str);
                    let val_type = map_go_type_str(val_str);
                    quote! { return ::gourd::GoMap::<#key_type, #val_type>::new() }
                } else {
                    quote! { return ::gourd::GoMap::<::gourd::GoString, i32>::new() }
                }
            }
            s if s.starts_with("[]") => {
                let slice_args: Vec<&str> = s.splitn(2, ',').collect();
                let slice_type_str = slice_args[0].trim().trim_start_matches("[]").trim();
                let slice_type = map_go_type_str(slice_type_str);
                if slice_args.len() == 2 {
                    let len_str = slice_args[1].trim();
                    let len: syn::LitInt = syn::parse_str(len_str).unwrap_or_else(
                        |_| syn::parse_quote!(0usize)
                    );
                    quote! { return ::std::iter::repeat(#slice_type::default()).take(#len).collect::<Vec::<#slice_type>>() }
                } else {
                    quote! { return ::std::iter::repeat(#slice_type::default()).take(0usize).collect::<Vec::<#slice_type>>() }
                }
            }
            _ => {
                let msg = format!("TODO: make with unsupported type: {}", raw_args);
                quote! { return { compile_error!(concat!("TODO: make with unsupported type: ", #msg)) } }
            }
        };
        stmts.push(GoStmt::RawStmt(make_rust));
        if input.peek(token::Semi) {
            let _semi: token::Semi = input.parse()?;
        }
        return Ok(true);
    }

    // Check for `return append(...)`
    let append_fork = input.fork();
    let saved_append = input.fork();
    let append_ident = append_fork.parse::<syn::Ident>().ok().map(|id| id.to_string());
    let append_paren = append_fork.peek(syn::token::Paren);
    let is_append = append_ident.as_deref() == Some("append") && append_paren;
    if is_append {
        input.advance_to(&saved_append);
        let _: syn::Ident = input.parse()?;
        let mut args_ts = TokenStream::new();
        let has_tt = input.cursor().token_tree();
        if let Some((proc_macro2::TokenTree::Group(group), _)) = has_tt {
            if group.delimiter() == proc_macro2::Delimiter::Parenthesis {
                args_ts.extend(group.stream());
                // CONSUME the paren group from the input stream
                let _group: proc_macro2::TokenTree = input.parse()?;
            }
        }
        if input.peek(token::Semi) {
            let _semi: token::Semi = input.parse()?;
        }
        let raw_args = args_ts.to_string();
        let (slice_str, items_str) = split_top_level_comma(&raw_args);
        if items_str.is_none() {
            // append(slice) with no items — return the slice as Vec for compatibility
            let slice = slice_str.trim();
            let rust_slice = go_to_rust_slice_arg(slice);
            stmts.push(GoStmt::RawStmt(quote! { return #rust_slice.to_vec() }));
        } else {
            let slice = slice_str.trim();
            let items_str = items_str.unwrap().trim();
            let rust_slice = go_to_rust_slice_arg(slice);
            let items: Vec<_> = split_top_level_items(items_str).into_iter().map(|item| {
                let item = item.trim();
                if item.is_empty() {
                    return quote! {};
                }
                if let Ok(lit) = syn::parse_str::<syn::LitInt>(item) {
                    quote! { __gourd_append_result.push(#lit); }
                } else if let Ok(ident) = syn::parse_str::<syn::Ident>(item) {
                    quote! { __gourd_append_result.push(#ident); }
                } else {
                    let msg = item.to_string();
                    quote! { __gourd_append_result.push({ compile_error!(concat!("TODO: append item: ", #msg)) }); }
                }
            }).collect();
            // Emit as two separate statements: the let binding, then return
            stmts.push(GoStmt::RawStmt(quote! {
                let __gourd_append_result = { let mut __gourd_append_result = #rust_slice.to_vec(); #(#items)* __gourd_append_result };
            }));
            stmts.push(GoStmt::RawStmt(quote! { return __gourd_append_result }));
        }
        return Ok(true);
    }

    // Check for `return []T{...}` slice literal in return
    let adv_fork = input.fork();
    
    if adv_fork.peek(syn::token::Bracket) {
        input.advance_to(&adv_fork);
        let _ts: proc_macro2::TokenTree = input.parse()?;
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
            // Parse the brace-delimited group (Go `{1, 2, 3}` in the token stream)
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
            // Use RawStmt with pre-transpiled return vec![] to avoid HIR tuple wrapping
            
            stmts.push(GoStmt::RawStmt(quote! { return vec![ #(#elems),* ] }));
            if input.peek(token::Semi) {
                let _semi: token::Semi = input.parse()?;
            }
            return Ok(true);
        }
    }

    // Check for type assertion: `return x.(T)`
    let after_ret = input.fork();
    if !after_ret.is_empty() {
        let check_str = after_ret.cursor().token_stream().to_string();
        let is_type_assertion = check_str.contains(".(");

        if is_type_assertion {
            input.advance_to(&after_ret);
            let receiver_ident: Ident = input.parse()?;
            let receiver = Expr::Path(syn::ExprPath { attrs: vec![], qself: None, path: syn::Path::from(receiver_ident) });

            let mut types: Vec<syn::Type> = Vec::new();
            loop {
                let next_fork = input.fork();
                if !next_fork.peek(syn::token::Dot) { break; }
                let _: proc_macro2::Punct = input.parse()?;
                let _: proc_macro2::Group = input.parse()?;
                let tfork = after_ret.fork();
                let mut all_groups: Vec<proc_macro2::Group> = Vec::new();
                let mut remaining = tfork;
                let _ = remaining.parse::<proc_macro2::TokenTree>()?;
                loop {
                    let gcheck = remaining.fork();
                    if let Ok(tt) = gcheck.parse::<proc_macro2::TokenTree>() {
                        if let proc_macro2::TokenTree::Punct(p) = tt {
                            if p.as_char() == '.' {
                                let gcheck2 = gcheck.fork();
                                if let Ok(tt2) = gcheck2.parse::<proc_macro2::TokenTree>() {
                                    if let proc_macro2::TokenTree::Group(g) = tt2 {
                                        all_groups.push(g);
                                        remaining = gcheck2;
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                    break;
                }
                for g in &all_groups {
                    if g.delimiter() == proc_macro2::Delimiter::Parenthesis {
                        if let Ok(tid) = syn::parse2::<syn::Ident>(g.stream()) {
                            let tname = tid.to_string();
                            let rt = match tname.as_str() {
                                "int" => "i32", "int8" => "i8", "int16" => "i16",
                                "int32" => "i32", "int64" => "i64",
                                "uint" => "u32", "uint8" => "u8", "uint16" => "u16",
                                "uint32" => "u32", "uint64" => "u64",
                                "uintptr" => "usize", "byte" => "u8",
                                "rune" => "char", "float32" => "f32",
                                "float64" => "f64", "bool" => "bool",
                                "string" => "String",
                                "error" => "Box<dyn std::error::Error>",
                                _ => "i32",
                            };
                            types.push(syn::parse_str(rt).unwrap());
                        }
                    }
                }
                if !all_groups.is_empty() {
                    input.advance_to(&remaining);
                }
            }

            if types.is_empty() {
                stmts.push(GoStmt::Expr(receiver));
            } else {
                for ty in types.into_iter().rev() {
                    stmts.push(GoStmt::GoTypeAssert(receiver.clone(), ty));
                }
            }
            if input.peek(token::Semi) {
                let _semi: token::Semi = input.parse()?;
            }
            return Ok(true);
        }

    }

    // Check for type assertion: `return x.(T)` - fall through to existing code
    let after_ret = input.fork();
    if !after_ret.is_empty() {
        let check_str = after_ret.cursor().token_stream().to_string();
        let is_type_assertion = check_str.contains(".(");

        if is_type_assertion {
            // Parse type assertion: `return x.(T)`
            let receiver_fork = after_ret.fork();
            if receiver_fork.parse::<Expr>().is_ok() {
                input.advance_to(&receiver_fork);
                let receiver = input.parse::<Expr>()?;

                let _dot: token::Dot = input.parse()?;

                // Parse parenthesized type(s): `(T)` or `(T1, T2)`
                let content;
                let _paren = syn::parenthesized!(content in input);

                let mut types: Vec<syn::Type> = Vec::new();
                if let Ok(ty) = content.parse::<syn::Type>() {
                    types.push(ty);
                }
                while content.peek(token::Comma) {
                    let _: token::Comma = content.parse()?;
                    if let Ok(ty) = content.parse::<syn::Type>() {
                        types.push(ty);
                    }
                }

                if types.is_empty() {
                    stmts.push(GoStmt::Expr(receiver));
                } else {
                    for ty in types.into_iter().rev() {
                        stmts.push(GoStmt::GoTypeAssert(receiver.clone(), ty));
                    }
                }
                if input.peek(token::Semi) {
                    let _semi: token::Semi = input.parse()?;
                }
                return Ok(true);
            }
        }
    }

    let expr_fork = after_ret.fork();
    if expr_fork.parse::<Expr>().is_ok() {
        input.advance_to(&after_ret);
        let first = input.parse::<Expr>()?;
        let multi_fork = input.fork();
        if multi_fork.peek(token::Comma) {
            let mut multi_exprs: Vec<Expr> = vec![first];
            input.parse::<token::Comma>()?;
            loop {
                if input.peek(syn::token::Brace) {
                    break;
                }
                let local_fork = input.fork();
                if local_fork.peek(syn::Ident) {
                    let kw_fork = local_fork.fork();
                    if let Ok(kw) = kw_fork.parse::<syn::Ident>() {
                        let kw_str = kw.to_string();
                        if matches!(kw_str.as_str(),
                            "if" | "for" | "return" | "case" | "default") {
                            break;
                        }
                    }
                }
                let next_fork = input.fork();
                if next_fork.parse::<Expr>().is_ok() {
                    let expr = input.parse::<Expr>()?;
                    multi_exprs.push(expr);
                    if input.peek(token::Comma) {
                        input.parse::<token::Comma>()?;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            stmts.push(GoStmt::GoReturn(multi_exprs));
        } else {
            // Single return value — push as GoReturn to get HirStatement::Return (no extra braces)
            stmts.push(GoStmt::GoReturn(vec![first]));
        }
        if input.peek(token::Semi) {
            let _semi: token::Semi = input.parse()?;
        }
        return Ok(true);
    }

    stmts.push(GoStmt::GoReturn(vec![]));
    if input.peek(token::Semi) {
        let _semi: token::Semi = input.parse()?;
    }
    Ok(true)
}
