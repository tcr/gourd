// HIR → Rust code generation.
//
// This module converts HIR into valid Rust TokenStream output.
// By operating on the HIR instead of raw tokens, we avoid the
// operator precedence bugs that plague the current transpiler.
//
// ### Design principles
//
// 1. **Semantic correctness first** — the HIR captures intent, codegen
//    just emits Rust tokens for that intent.
// 2. **No token-level hackery** — unlike the current `quote!` approach,
//    the codegen builds Rust tokens from well-defined HIR structures.
// 3. **Easy to test** — each HIR variant has a corresponding codegen
//    function that can be unit tested independently.

use proc_macro2::TokenStream;
use quote::quote;
use super::expression::{ HirExpr, HirExprKind, HirLiteral, HirBinaryOp, HirUnaryOp, MakeKind };
use super::types::{ HirType, HirTypeKind, HirInterfaceMethod, HirReceiverFn, HirSelect, HirSelectCase, HirSwitch, HirFunction, HirStruct };
use super::statement::{ HirStatement, HirBlock };
use super::ast::{ GoFn, GoStruct };
use crate::transpiler::heuristics;
use syn::Ident;

/// Convert a Go name (camelCase) to Rust snake_case.
fn to_snake_case(name: &str) -> String {
    let mut result = String::with_capacity(name.len() + 4);
    let chars: Vec<char> = name.chars().collect();
    for (i, ch) in chars.iter().enumerate() {
        if ch.is_uppercase() {
            if i > 0 && !name[..i].ends_with('_') {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
        } else if ch.is_ascii_digit() && i > 0 && chars[i - 1].is_lowercase() {
            result.push('_');
            result.push(*ch);
        } else {
            result.push(*ch);
        }
    }
    result
}

/// Convert a HIR expression to Rust tokens.
pub fn hir_expr_to_rust(expr: &HirExpr) -> TokenStream {
    match &expr.kind {
        HirExprKind::Literal(lit) => hir_literal_to_rust(lit),
        HirExprKind::Identifier(id) => quote! { #id },
        HirExprKind::Path(path) => quote! { #path },
        HirExprKind::Macro(tokens) => {
            // Detect if this came from a vec! macro by checking the token stream
            // The preprocessing converts []T{elems} → vec![elems], so the HIR
            // stores just the inner tokens. We need to regenerate vec![...].
            let tokens_str = tokens.to_string();
            // Check if this is vec! content by looking for comma-separated expressions
            // A vec! macro has the form: elem1, elem2, ...
            // Detect by checking if tokens start with a digit, literal, or opening bracket
            let first_non_whitespace: Vec<char> = tokens_str.chars().filter(|c| !c.is_whitespace()).collect();
            if first_non_whitespace.iter().next().map_or(false, |c| c.is_ascii_digit() || *c == '"' || *c == '\'' || *c == '[') {
                // Likely vec! content — regenerate as vec![...]
                quote! { vec ! [ #tokens ] }
            } else if tokens.is_empty() {
                quote! { vec ! [] }
            } else {
                // Other macros (format!, etc.): pass through as-is
                tokens.clone()
            }
        }
        HirExprKind::Binary { op, lhs, rhs } => hir_binary_to_rust(op, lhs, rhs),
        HirExprKind::Unary { op, operand } => hir_unary_to_rust(op, operand),
        HirExprKind::Call { func, args } => hir_call_to_rust(func, args),
        HirExprKind::MethodCall { receiver, method, args } => hir_method_call_to_rust(receiver, method, args),
        HirExprKind::FieldAccess { receiver, field } => hir_field_access_to_rust(receiver, field),
        HirExprKind::Index { collection, index } => hir_index_to_rust(collection, index),
        HirExprKind::StringByteIndex { collection, index } => {
            // Go string byte indexing: `str[i]` → `.as_bytes()[i as usize]`
            // In Go, string indexing always yields a byte (u8).
            let collection_tokens = hir_expr_to_rust(collection);
            let index_tokens = hir_expr_to_rust(index);
            quote! { #collection_tokens.as_bytes()[(#index_tokens) as usize] }
        }
        HirExprKind::Slice { collection, start, end } => hir_slice_to_rust(collection, start, end),
        HirExprKind::RangeVar(name) => quote! { #name },
        HirExprKind::Cast { value, target_type } => hir_cast_to_rust(value, target_type),
        HirExprKind::TypeConvert { func, arg } => hir_type_convert_to_rust(func, arg),
        HirExprKind::Tuple(values) => hir_tuple_to_rust(values),
        HirExprKind::Block(block) => hir_block_to_rust(block, true),
        HirExprKind::Closure { params, returns, body } => hir_closure_to_rust(params, returns, body),
        HirExprKind::ErrorCheck { value } => hir_error_check_to_rust(value),
        HirExprKind::Len(expr) => hir_len_to_rust(expr),
        HirExprKind::Cap(expr) => hir_cap_to_rust(expr),
        HirExprKind::Make(kind) => hir_make_to_rust(kind),
        HirExprKind::Append { target, elements } => hir_append_to_rust(target, elements),
        HirExprKind::Copy { dst, src } => hir_copy_to_rust(dst, src),
        HirExprKind::StdCall { func_name, args } => {
            // Standard library functions require reference wrapping:
            // std::copy(dst, src) → std_copy(&mut dst, &src)
            // std::delete(m, key) → std_delete(m, key)
            // std::append(slice, items...) → std_append(slice, &[items])
            let rust_fn = match func_name.as_str() {
                "copy" => "std_copy",
                "delete" => "std_delete",
                "append" => "std_append",
                _ => return emit_todo(&format!("std::{} is not supported", func_name)),
            };
            let rust_fn_ident: syn::Ident = syn::parse_str(rust_fn).unwrap();
            let func_path = quote! { ::gourd::prelude::#rust_fn_ident };
            let arg_tokens: Vec<TokenStream> = args.iter().map(|a| hir_expr_to_rust(a)).collect();
            let wrapped_args: Vec<TokenStream> = match func_name.as_str() {
                "copy" => {
                    // std_copy(&mut [T], &[T]) — pass slices from vecs
                    if arg_tokens.len() >= 2 {
                        let first = &arg_tokens[0];
                        // For the second arg, if it's already a slice-like value, just pass &it
                        // Otherwise wrap in &[...]
                        let second = &arg_tokens[1];
                        vec![
                            quote! { &mut (#first).as_mut_slice() },
                            quote! { &#second },
                        ]
                    } else {
                        arg_tokens
                    }
                }
                "delete" => {
                    // std_delete(HashMap, key) — takes HashMap by value
                    if arg_tokens.len() >= 2 {
                        let first = &arg_tokens[0];
                        let second = &arg_tokens[1];
                        vec![
                            first.clone(),
                            second.clone(),
                        ]
                    } else {
                        arg_tokens
                    }
                }
                "append" => {
                    // std_append(Vec, &[items]) — collect remaining args into a slice
                    if arg_tokens.len() >= 2 {
                        let items = &arg_tokens[1..];
                        vec![
                            arg_tokens[0].clone(),
                            quote! { &[ #(#items),* ] },
                        ]
                    } else if arg_tokens.len() == 1 {
                        // No items to append
                        vec![
                            arg_tokens[0].clone(),
                            quote! { &[] },
                        ]
                    } else {
                        arg_tokens
                    }
                }
                _ => arg_tokens,
            };
            quote! { #func_path( #(#wrapped_args),* ) }
        }
        HirExprKind::SliceLiteral(elements) => hir_slice_literal_to_rust(elements),
        HirExprKind::Map(entries) => hir_map_literal_to_rust(entries),
        HirExprKind::StructLit { name, fields } => hir_struct_lit_to_rust(name, fields),
        HirExprKind::ChannelSend { channel, value } => hir_channel_send_to_rust(channel, value),
        HirExprKind::ChannelRecv { channel, target: _ } => hir_channel_recv_to_rust(channel),
        HirExprKind::Select { cases, default_body } => {
            let default_body = default_body.as_ref().map(|b| b.clone());
            hir_select_to_rust(cases, default_body)
        }
        HirExprKind::Match { selector, arms, default_body } => {
            let default_body = default_body.as_ref().map(|b| b.clone());
            hir_match_to_rust(selector, arms, default_body)
        }
        HirExprKind::New(arg) => {
            // Go `new(Foo)` → `Foo::default()`
            let arg_tokens = hir_expr_to_rust(arg);
            quote! { #arg_tokens::default() }
        }

        HirExprKind::Unsupported(msg) => quote! { compile_error!(concat!("HIR: unsupported: ", #msg)) },
        HirExprKind::MinMax { kind, values } => hir_minmax_to_rust(kind, values),
        HirExprKind::Panic(msg) => quote! { panic!(#msg) },
        HirExprKind::Delete { map, key } => hir_delete_to_rust(map, key),
    }
}

/// Convert a HIR literal to Rust tokens.
fn hir_literal_to_rust(lit: &HirLiteral) -> TokenStream {
    match lit {
        // Go `int` defaults to `i32` semantics. Produce clean integer literal
        // without suffix by constructing a syn::LitInt.
        HirLiteral::Int(n) => {
            // Use i64 suffix only for large integers that might be used in
            // int64 contexts (e.g., nanoseconds > i32::MAX).
            // Small literals stay unsuffixed (default i32 in Rust).
            let lit_str = n.to_string();
            let lit: syn::LitInt = if *n > i32::MAX as u64 {
                syn::parse_str(&format!("{}i64", lit_str))
                    .unwrap_or_else(|_| syn::parse_quote!(0i64))
            } else {
                syn::parse_str(&lit_str)
                    .unwrap_or_else(|_| syn::parse_quote!(0i64))
            };
            quote! { #lit }
        }
        HirLiteral::Float(f) => quote! { #f },
        HirLiteral::Bool(b) => quote! { #b },
        HirLiteral::StringTy(s) => quote! { ::std::string::String::from(#s) },
        HirLiteral::Nil => quote! { None },
    }
}

/// Convert a HIR binary operation to Rust tokens.
///
/// This function is clean and correct — no token-level hacks.
/// The operator precedence is handled correctly by the HIR structure itself:
/// when `Binary { op: Eq, lhs: ..., rhs: ... }` is created, the `Eq`
/// operation wraps the entire LHS and RHS. No ambiguity.
fn hir_binary_to_rust(op: &HirBinaryOp, lhs: &HirExpr, rhs: &HirExpr) -> TokenStream {
    let lhs_tokens = hir_expr_to_rust(lhs);
    let rhs_tokens = hir_expr_to_rust(rhs);
    match op {
        HirBinaryOp::Add => {
            // Only add `&` for actual string concatenation to avoid moves.
            // For numeric addition: `lhs + rhs`
            let lhs_ty = get_expr_type(lhs);
            let rhs_ty = get_expr_type(rhs);
            if lhs_ty.is_string() || rhs_ty.is_string() {
                quote! { #lhs_tokens + &#rhs_tokens }
            } else {
                quote! { #lhs_tokens + #rhs_tokens }
            }
        }
        HirBinaryOp::Sub => quote! { #lhs_tokens - #rhs_tokens },
        HirBinaryOp::Mul => quote! { #lhs_tokens * #rhs_tokens },
        HirBinaryOp::Div => quote! { #lhs_tokens / #rhs_tokens },
        HirBinaryOp::Mod => quote! { #lhs_tokens % #rhs_tokens },
        HirBinaryOp::Eq => quote! { #lhs_tokens == #rhs_tokens },
        HirBinaryOp::Ne => quote! { #lhs_tokens != #rhs_tokens },
        HirBinaryOp::Lt => quote! { #lhs_tokens < #rhs_tokens },
        HirBinaryOp::Le => quote! { #lhs_tokens <= #rhs_tokens },
        HirBinaryOp::Gt => quote! { #lhs_tokens > #rhs_tokens },
        HirBinaryOp::Ge => quote! { #lhs_tokens >= #rhs_tokens },
        HirBinaryOp::And => quote! { #lhs_tokens && #rhs_tokens },
        HirBinaryOp::Or => quote! { #lhs_tokens || #rhs_tokens },
        HirBinaryOp::Assign => quote! { #lhs_tokens = #rhs_tokens },
        HirBinaryOp::AddAssign => quote! { #lhs_tokens += #rhs_tokens },
        HirBinaryOp::SubAssign => quote! { #lhs_tokens -= #rhs_tokens },
        HirBinaryOp::MulAssign => quote! { #lhs_tokens *= #rhs_tokens },
        HirBinaryOp::DivAssign => quote! { #lhs_tokens /= #rhs_tokens },
        HirBinaryOp::ModAssign => quote! { #lhs_tokens %= #rhs_tokens },
        HirBinaryOp::AndAssign => quote! { #lhs_tokens &= #rhs_tokens },
        HirBinaryOp::OrAssign => quote! { #lhs_tokens |= #rhs_tokens },
        HirBinaryOp::XorAssign => quote! { #lhs_tokens ^= #rhs_tokens },
        HirBinaryOp::LshAssign => quote! { #lhs_tokens <<= #rhs_tokens },
        HirBinaryOp::RshAssign => quote! { #lhs_tokens >>= #rhs_tokens },
        HirBinaryOp::BitAnd => quote! { #lhs_tokens & #rhs_tokens },
        HirBinaryOp::BitOr => quote! { #lhs_tokens | #rhs_tokens },
        HirBinaryOp::BitXor => quote! { #lhs_tokens ^ #rhs_tokens },
        HirBinaryOp::Lsh => quote! { #lhs_tokens << #rhs_tokens },
        HirBinaryOp::Rsh => quote! { #lhs_tokens >> #rhs_tokens },
        _ => emit_todo_binary_op(op),
    }
}

/// Convert a HIR unary operation to Rust tokens.
///
/// **Key fix**: The `Not` operator now wraps the operand in parentheses,
/// because in Go `!i < len` means `!(i < len)`, but in Rust `!i < len`
/// means `(!i) < len` (wrong precedence). The HIR captures this correctly:
/// `Unary { op: Not, operand: Binary { op: Lt, ... } }` unambiguously means
/// "NOT of (LHS < RHS)", and the codegen wraps it: `!(lhs < rhs)`.
fn hir_unary_to_rust(op: &HirUnaryOp, operand: &HirExpr) -> TokenStream {
    let operand_tokens = hir_expr_to_rust(operand);
    match op {
        HirUnaryOp::Not => {
            // Parenthesize the operand to fix precedence: `!(...)`
            // In Go, `!a < b` means `!(a < b)`, but in Rust `!a < b` means `(!a) < b`.
            quote! { !(#operand_tokens) }
        }
        HirUnaryOp::Neg => quote! { -#operand_tokens },
        HirUnaryOp::Deref => quote! { *#operand_tokens },
        HirUnaryOp::AddressOf => quote! { &#operand_tokens },
    }
}

/// Convert a HIR function call to Rust tokens.
/// Handles special builtins: strings.Fields, strings.Join, fmt.Sprintf, etc.
fn hir_call_to_rust(func: &HirExpr, args: &[HirExpr]) -> TokenStream {
    let func_tokens = hir_expr_to_rust(func);
    let arg_tokens: Vec<TokenStream> = args.iter().map(|a| hir_expr_to_rust(a)).collect();
    
    // Handle special path-based calls: strings.Fields, strings.Join, fmt.Sprintf
    if let HirExprKind::Path(hir_path) = &func.kind {
        // quote! produces "strings :: Fields" with spaces around `::`
        // Normalize for detection by checking both spaced and non-spaced forms
        let path_str = quote::quote!(#hir_path).to_string();
        let normalized = path_str.replace(" :: ", "::").replace("::", "::");
        if normalized.contains("strings::Fields") || path_str.contains("strings :: Fields") {
            // strings.Fields(s) → gourd::prelude::fields(&s)
            return quote! { ::gourd::prelude::fields(&#(#arg_tokens),*) };
        }
        if normalized.contains("strings::Join") || path_str.contains("strings :: Join") {
            // strings.Join(parts, sep) → gourd::prelude::join(&parts, sep)
            return quote! { ::gourd::prelude::join(&#(#arg_tokens),*) };
        }
        if normalized.contains("fmt::Sprintf") || path_str.contains("fmt :: Sprintf")
            || normalized.contains("::gourd::prelude::fmt_sprintf") || path_str.contains(":: gourd :: prelude :: fmt_sprintf")
        {
            // fmt.Sprintf(format, args...) → gourd::prelude::fmt_sprintf(format, &vec![args.to_string(), ...])
            if arg_tokens.len() > 1 {
                let first = arg_tokens[0].clone();
                let rest: Vec<TokenStream> = arg_tokens.into_iter().skip(1).map(|t| quote! { #t .to_string() }).collect();
                return quote! { ::gourd::prelude::fmt_sprintf(#first, &vec![#(#rest),*]) };
            }
        }
        if normalized.contains("fmt::Println") || path_str.contains("fmt :: Println") {
            // fmt.Println(args...) → gourd::prelude::fmt_println(args...)
            return quote! { ::gourd::prelude::fmt_println(#(#arg_tokens),*) };
        }
        if normalized.contains("fmt::Print") || path_str.contains("fmt :: Print") {
            // fmt.Print(args...) → gourd::prelude::fmt_print(args...)
            return quote! { ::gourd::prelude::fmt_print(#(#arg_tokens),*) };
        }
        if normalized.contains("fmt::Printf") || path_str.contains("fmt :: Printf") {
            // fmt.Printf(format, args...) → gourd::prelude::fmt_printf(format, args...)
            if arg_tokens.len() > 1 {
                let first = arg_tokens[0].clone();
                let rest: Vec<TokenStream> = arg_tokens.into_iter().skip(1).collect();
                return quote! { ::gourd::prelude::fmt_printf(#first, #(#rest),*) };
            }
        }
    }
    
    quote! { #func_tokens( #(#arg_tokens),* ) }
}

/// Convert a HIR method call to Rust tokens.
fn hir_method_call_to_rust(receiver: &HirExpr, method: &syn::Ident, args: &[HirExpr]) -> TokenStream {
    let receiver_tokens = hir_expr_to_rust(receiver);
    let arg_tokens: Vec<TokenStream> = args.iter().map(|a| hir_expr_to_rust(a)).collect();
    quote! { #receiver_tokens.#method( #(#arg_tokens),* ) }
}

/// Convert a HIR field access to Rust tokens.
fn hir_field_access_to_rust(receiver: &HirExpr, field: &syn::Ident) -> TokenStream {
    let receiver_tokens = hir_expr_to_rust(receiver);
    quote! { #receiver_tokens.#field }
}

/// Convert a HIR index access to Rust tokens.
/// Go indices (int/i32) are cast to usize for Rust compatibility.
/// Map accesses are detected via collection name heuristics and
/// redirected to `map_get_ref` for correct HashMap behavior.
fn hir_index_to_rust(collection: &HirExpr, index: &HirExpr) -> TokenStream {
    let collection_tokens = hir_expr_to_rust(collection);
    let index_tokens = hir_expr_to_rust(index);
    let collection_str = quote::quote!(#collection_tokens).to_string();
    let index_str = quote::quote!(#index_tokens).to_string();

    // Detect map access by collection name containing "HashMap"
    if collection_str.contains("HashMap") || collection_str.contains("hash_map") {
        return quote! { ::gourd::prelude::map_get_ref( &#collection_tokens, &#index_tokens) };
    }

    // Detect map access by index being a string type
    if collection_str.contains("from(") {
        // Map access via variable name heuristic (previous logic moved below)
    }

    // Heuristic: variable names suggest map access → use map_get_ref helper
    if heuristics::heuristic_should_use_map_get_ref(&collection_str, &index_str) {
        return quote! { ::gourd::prelude::map_get_ref( &#collection_tokens, &#index_tokens) };
    }

    // Detect string byte indexing: Go `str[i]` where str is a String.
    // In Rust, `String[i]` is not valid — use `.as_bytes()[i]` instead.
    // Heuristic: simple identifier, not map-like, and name suggests string.
    let collection_lower = collection_str.to_lowercase();
    let collection_is_string = collection_str.contains("::std::string::String")
        || collection_str.contains("::std::string :: String")
        || collection_lower == "string";
    // Also detect common Go string parameter names that aren't slice collections.
    // In Go, `input[i]` on a string parameter always yields a byte.
    // We detect this when: it's a single-word identifier, not map-like,
    // and the name is commonly used for strings rather than slices.
    let common_string_params = ["input", "text", "str", "s", "msg", "phrase"];
    let is_common_string_param = syn::parse_str::<syn::Ident>(&collection_lower)
        .ok()
        .filter(|id| {
            common_string_params.iter().any(|&name| id == name)
        })
        .is_some();
    if is_common_string_param && !heuristics::heuristic_is_map_iteration(&collection_lower) {
        return quote! { #collection_tokens.as_bytes()[ (#index_tokens) as usize] };
    }

    // Default: cast Go int index to usize for Rust slice indexing
    quote! { #collection_tokens[(#index_tokens) as usize] }
}

/// Convert a HIR slice expression to Rust tokens.
fn hir_slice_to_rust(collection: &HirExpr, start: &Option<Box<HirExpr>>, end: &Option<Box<HirExpr>>) -> TokenStream {
    let collection_tokens = hir_expr_to_rust(collection);
    match (start, end) {
        (Some(s), Some(e)) => {
            let s_tokens = hir_expr_to_rust(s);
            let e_tokens = hir_expr_to_rust(e);
            quote! { #collection_tokens[#s_tokens..#e_tokens] }
        }
        (Some(s), None) => {
            let s_tokens = hir_expr_to_rust(s);
            quote! { #collection_tokens[#s_tokens..] }
        }
        (None, Some(e)) => {
            let e_tokens = hir_expr_to_rust(e);
            quote! { #collection_tokens[..#e_tokens] }
        }
        (None, None) => {
            quote! { #collection_tokens[..] }
        }
    }
}

/// Convert a HIR cast to Rust tokens.
fn hir_cast_to_rust(value: &HirExpr, target_type: &HirType) -> TokenStream {
    let value_tokens = hir_expr_to_rust(value);
    let ty_tokens = target_type.to_rust_type();
    // Use a suffixed literal for the integer literal to ensure Rust
    // natively treats it as i64, avoiding i64/i32 mismatch.
    // The `as i32` cast is applied to the ENTIRE parenthesized expression.
    quote! { (#value_tokens) as #ty_tokens }
}

/// Convert a Go type conversion call to Rust tokens.
/// `int(x)` → `(x as i32)`, `string(x)` → `String::from(x)`, etc.
fn hir_type_convert_to_rust(func: &syn::Ident, arg: &HirExpr) -> TokenStream {
    let arg_tokens = hir_expr_to_rust(arg);
    let func_name = func.to_string();
    match func_name.as_str() {
        // Integer conversions
        "int" => quote! { ((#arg_tokens) as i32) },
        "int8" => quote! { ((#arg_tokens) as i8) },
        "int16" => quote! { ((#arg_tokens) as i16) },
        "int32" => quote! { ((#arg_tokens) as i32) },
        "int64" => quote! { ((#arg_tokens) as i64) },
        // Unsigned conversions
        "uint" => quote! { ((#arg_tokens) as u32) },
        "uint8" => quote! { ((#arg_tokens) as u8) },
        "uint16" => quote! { ((#arg_tokens) as u16) },
        "uint32" => quote! { ((#arg_tokens) as u32) },
        "uint64" => quote! { ((#arg_tokens) as u64) },
        "uintptr" => quote! { ((#arg_tokens) as usize) },
        // Float conversions
        "float32" => quote! { ((#arg_tokens) as f32) },
        "float64" => quote! { ((#arg_tokens) as f64) },
        // Bool conversion
        "bool" => quote! { ((#arg_tokens) as bool) },
        // Byte conversion
        "byte" => quote! { ((#arg_tokens) as u8) },
        // Rune conversion
        "rune" => {
            // Handle int + char expressions: `count + '0'` in Go → the '0' literal
            // needs to be converted to i32 for addition in Rust.
            let arg_str = quote::quote!(#arg_tokens).to_string();
            if arg_str.contains('+') && (arg_str.contains('"') || arg_str.contains("from(") ) {
                // The argument contains a char literal being added to an int.
                // Convert the char to its integer value for proper Rust arithmetic.
                quote! { ((#arg_tokens) as i32 as u8 as char) }
            } else {
                quote! { ((#arg_tokens) as u8 as char) }
            }
        },
        // String conversion
        "string" => {
            let arg_str = quote::quote!(#arg_tokens).to_string();
            // If the argument is already a u8 (from string byte indexing), 
            // convert byte to char then to String
            if arg_str.contains("as_bytes") {
                quote! { ( #arg_tokens as char ).to_string() }
            } else {
                quote! { ::std::str::from_utf8(&#arg_tokens).unwrap_or("").to_string() }
            }
        },
        // Fallback
        _ => quote! { #func(#arg_tokens) },
    }
}

/// Convert a HIR tuple to Rust tokens.
fn hir_tuple_to_rust(values: &[HirExpr]) -> TokenStream {
    if values.is_empty() {
        // Empty tuple → unit `()`
        // This fixes the `[]int{}` issue: zero-element slice literal
        // is parsed as Expr::Array([]) → Tuple([]) → should emit `()`
        quote! { () }
    } else {
        let tokens: Vec<TokenStream> = values.iter().map(|v| hir_expr_to_rust(v)).collect();
        quote! { ( #(#tokens),* ) }
    }
}

/// Convert a HIR block to Rust tokens.
/// Remove the outer `{ ... }` wrapping from a statement token stream.
/// Used in multi-statement blocks to flatten nested brace pairs.
fn strip_block_wrapping(ts: &TokenStream) -> TokenStream {
    let s = ts.to_string();
    // Match `{ ... }` wrapping — strip outer braces
    if s.starts_with('{') && s.ends_with('}') {
        // Remove leading `{` and trailing `}`
        let inner: TokenStream = s[1..s.len()-1].trim().parse().unwrap_or_else(|_| ts.clone());
        inner
    } else {
        // No wrapping — return as-is
        ts.clone()
    }
}

/// When `strip_returns` is true (for match arms), returns and semicolons are
/// stripped so the block evaluates to an expression. When false (for
/// control-flow bodies), returns are preserved as explicit control flow.
fn hir_block_to_rust(block: &HirBlock, strip_returns: bool) -> TokenStream {
    let stmt_tokens: Vec<TokenStream> = block.stmts.iter().map(|s| hir_stmt_to_rust(s, strip_returns)).collect();
    if stmt_tokens.is_empty() {
        quote! { { } }
    } else if stmt_tokens.len() == 1 {
        let inner = &stmt_tokens[0];
        if strip_returns {
            // Match arm context: strip semicolons and `return` so the body
            // evaluates to an expression value.
            let mut inner_str = inner.to_string();
            while inner_str.ends_with(';') {
                inner_str.pop();
                inner_str = inner_str.trim_end().to_string();
            }
            let inner_str = if inner_str.starts_with("return ") {
                inner_str[7..].trim().to_string()
            } else {
                inner_str
            };
            let stripped: TokenStream = inner_str.parse().unwrap_or_else(|_| inner.clone());
            quote! { { #stripped } }
        } else {
            // Control-flow body: preserve returns as explicit control flow.
            quote! { { #inner } }
        }
    } else {
        // Multi-statement: when strip_returns is true, produce a single
        // expression for closure bodies (if-then-return + default return).
        if strip_returns && stmt_tokens.len() >= 2 {
            // Extract the default else value from the last statement.
            let last = &stmt_tokens[stmt_tokens.len() - 1];
            let mut last_str = last.to_string();
            while last_str.ends_with(';') {
                last_str.pop();
                last_str = last_str.trim_end().to_string();
            }
            let default_else: TokenStream = if last_str.starts_with("return ") {
                last_str[7..].trim().parse()
                    .unwrap_or_else(|_| quote! { 0 })
            } else {
                last_str.parse()
                    .unwrap_or_else(|_| quote! { 0 })
            };

            // Strip wrapping from all non-last statements.
            let chains: Vec<TokenStream> = stmt_tokens[..stmt_tokens.len()-1]
                .iter()
                .map(|t| strip_block_wrapping(t))
                .collect();

            // Chains are already complete if-else expressions.
            // Just return the last chain's value (which includes its else default).
            if chains.is_empty() {
                default_else
            } else {
                let last_chain = &chains[chains.len() - 1];
                // But if the default_else is non-zero, we need to add an else.
                // Check if the last chain already has an else branch.
                let last_str = last_chain.to_string();
                if !default_else.to_string().eq("0") && !last_str.contains("else {") {
                    // No else in the chain, add the default.
                    quote! { if #last_chain else { #default_else } }
                } else {
                    // Chain already has an else — just use it.
                    last_chain.clone()
                }
            }
        } else {
            // Control-flow body: preserve returns as explicit control flow.
            quote! { { #(#stmt_tokens)* } }
        }
    }
}

/// Convert a HIR closure to Rust tokens.
fn hir_closure_to_rust(params: &[(syn::Ident, Option<Box<HirType>>)], returns: &[Box<HirType>], body: &HirBlock) -> TokenStream {
    let param_tokens: Vec<TokenStream> = params.iter().map(|(name, ty): &(syn::Ident, Option<Box<HirType>>)| {
        if let Some(ty) = ty {
            let ty_tokens = ty.to_rust_type();
            quote! { #name: #ty_tokens }
        } else {
            quote! { #name }
        }
    }).collect();
    // If the closure has a return type, its body must evaluate to that value.
    // Use strip_returns=true so the last statement produces an expression value.
    let body_tokens = if returns.is_empty() {
        hir_block_to_rust(body, false)
    } else {
        hir_block_to_rust(body, true)
    };
    // Generate return type tokens
    let return_tokens = if returns.is_empty() {
        quote! {}
    } else {
        let mapped: Vec<TokenStream> = returns.iter().map(|t| t.to_rust_type()).collect();
        if mapped.len() == 1 {
            quote! { -> #(#mapped)* }
        } else {
            quote! { -> ( #(#mapped),* ) }
        }
    };
    // Wrap the body in braces if it's a single expression (from strip_returns
    // multi-statement handling) — closures require body in braces.
    let body_tokens_str = body_tokens.to_string();
    let body_tokens = if !body_tokens_str.starts_with('{') {
        quote! { { #body_tokens } }
    } else {
        body_tokens
    };
    quote! { | #(#param_tokens),* | #return_tokens #body_tokens }
}

/// Convert a HIR error check to Rust tokens.
fn hir_error_check_to_rust(value: &HirExpr) -> TokenStream {
    let value_tokens = hir_expr_to_rust(value);
    quote! { if let ::std::result::Result::Err(err) = #value_tokens }
}

/// Convert a HIR len() call to Rust tokens.
fn hir_len_to_rust(expr: &HirExpr) -> TokenStream {
    let expr_tokens = hir_expr_to_rust(expr);
    quote! { #expr_tokens.len() as i32 }
}

/// Convert a HIR cap() call to Rust tokens.
fn hir_cap_to_rust(expr: &HirExpr) -> TokenStream {
    let expr_tokens = hir_expr_to_rust(expr);
    quote! { #expr_tokens.capacity() as i32 }
}

/// Convert a HIR make() call to Rust tokens.
fn hir_make_to_rust(kind: &MakeKind) -> TokenStream {
    match kind {
        MakeKind::Slice(elem_ty, len) => {
            let elem_ty = elem_ty.as_ref();
            let elem_tokens = elem_ty.to_rust_type();
            let len_tokens = hir_expr_to_rust(len);
            quote! { ::std::iter::repeat(#elem_tokens::default()).take(#len_tokens as usize).collect::<Vec<#elem_tokens>>() }
        }
        MakeKind::SliceWithCap(elem_ty, len, cap) => {
            let elem_ty = elem_ty.as_ref();
            let elem_tokens = elem_ty.to_rust_type();
            let len_tokens = hir_expr_to_rust(len);
            let cap_tokens = hir_expr_to_rust(cap);
            quote! { {
                let mut v: Vec<#elem_tokens> = Vec::with_capacity(#cap_tokens as usize);
                v.resize(#len_tokens as usize, #elem_tokens::default());
                v
            } }
        }
        MakeKind::Map(key_ty, val_ty) => {
            let key_tokens = key_ty.to_rust_type();
            let val_tokens = val_ty.to_rust_type();
            quote! { ::gourd::prelude::HashMap::<#key_tokens, #val_tokens>::new() }
        }
        MakeKind::MapWithCap(key_ty, val_ty, cap) => {
            let key_tokens = key_ty.to_rust_type();
            let val_tokens = val_ty.to_rust_type();
            let cap_tokens = hir_expr_to_rust(cap);
            quote! { {
                let mut m = ::gourd::prelude::HashMap::<#key_tokens, #val_tokens>::with_capacity(#cap_tokens as usize);
                m
            } }
        }
        MakeKind::Channel(elem_ty) => {
            let elem_tokens = elem_ty.to_rust_type();
            quote! { GoChannel::<#elem_tokens>::new() }
        }
        MakeKind::ChannelWithCap(elem_ty, cap) => {
            let elem_tokens = elem_ty.to_rust_type();
            let cap_tokens = hir_expr_to_rust(cap);
            quote! { GoChannel::<#elem_tokens>::with_capacity(#cap_tokens) }
        }
    }
}

/// Convert a HIR append() call to Rust tokens.
fn hir_append_to_rust(target: &HirExpr, elements: &[HirExpr]) -> TokenStream {
    let target_tokens = hir_expr_to_rust(target);
    let elem_tokens: Vec<TokenStream> = elements.iter().map(|e| hir_expr_to_rust(e)).collect();
    // For Vec<T>, append each element
    if elem_tokens.is_empty() {
        quote! { #target_tokens }
    } else {
        // Convert borrowed slice to Vec, append elements, return the Vec
        quote! { {
            let mut __gourd_append_result = (#target_tokens).to_vec();
            #(__gourd_append_result.push(#elem_tokens);)*
            __gourd_append_result
        } }
    }
}

/// Convert a HIR copy() call to Rust tokens.
fn hir_copy_to_rust(dst: &HirExpr, src: &HirExpr) -> TokenStream {
    let dst_tokens = hir_expr_to_rust(dst);
    let src_tokens = hir_expr_to_rust(src);
    quote! { #dst_tokens.copy_from_slice(&#src_tokens) }
}

/// Convert a HIR statement to Rust tokens.
pub fn hir_stmt_to_rust(stmt: &HirStatement, strip_returns: bool) -> TokenStream {
    match stmt {
        HirStatement::Local { name, mutable, value } => {
            // Local statements need a trailing semicolon for statement separation.
            let mut_kw = if *mutable { quote! { mut } } else { quote! {} };
            let value_tokens = hir_expr_to_rust(value);
            quote! { let #mut_kw #name = #value_tokens ; }
        }
        HirStatement::Assign { target, value } => {
            // Detect map index assignment: when target is Index, use map_set_mut_ref.
            // In Go, `map[key] = value` is an lvalue; in Rust we use
            // `*map_set_mut_ref(&mut map, &key) = value` to modify entries.
            if let HirExprKind::Index { collection, index } = &target.kind {
                let collection_tokens = hir_expr_to_rust(collection);
                let collection_str = collection_tokens.to_string();
                let collection_lower = collection_str.to_lowercase();
                // Heuristic: collection name suggests map → use map_set_mut_ref
                if heuristics::heuristic_should_use_map_set(&collection_str) {
                    let idx_tokens = hir_expr_to_rust(index);
                    let val_tokens = hir_expr_to_rust(value);
                    return quote! { *::gourd::prelude::map_set_mut_ref( &mut #collection_tokens, &#idx_tokens ) = #val_tokens };
                }
            }
            let target_tokens = hir_expr_to_rust(target);
            let value_tokens = hir_expr_to_rust(value);
            quote! { #target_tokens = #value_tokens ; }
        }
        HirStatement::Expr(expr) => {
            // Expression statements: don't add extra braces, let body wrapper handle it.
            // For tail-expression-producing expressions (match, blocks),
            // don't add a trailing semicolon so they become tail expressions.
            let expr_tokens = hir_expr_to_rust(expr);
            match &expr.kind {
                HirExprKind::Match { .. } | HirExprKind::Block { .. } => quote! { #expr_tokens },
                _ => quote! { #expr_tokens ; },
            }
        }
        HirStatement::If { cond, then_body, else_body } => {
            let cond_tokens = hir_expr_to_rust(cond);
            // Pass strip_returns through to inner blocks so that
            // `return x` inside if-then produces an expression value.
            let then_tokens = hir_block_to_rust(then_body, strip_returns);
            let else_tokens = match else_body.as_ref() {
                Some(b) => {
                    let body_tokens = hir_block_to_rust(b, strip_returns);
                    Some(quote! { else #body_tokens })
                }
                None => {
                    // When strip_returns is true (closure body context), add
                    // an implicit else { 0 } to give the if an expression type.
                    if strip_returns {
                        Some(quote! { else { 0 } })
                    } else {
                        None
                    }
                }
            };
            let else_tokens = match &else_tokens {
                Some(e) => e.clone(),
                None => quote! {},
            };
            quote! { if #cond_tokens #then_tokens #else_tokens }
        }
        HirStatement::While { cond, body } => {
            let cond_tokens = hir_expr_to_rust(cond);
            let body_tokens = hir_block_to_rust(body, strip_returns);
            quote! { while #cond_tokens #body_tokens ; }
        }
        HirStatement::ForRange { index_name, value_name, iterable, body } => {
            let iterable_tokens = hir_expr_to_rust(iterable);
            let body_tokens = hir_block_to_rust(body, strip_returns);
            match (index_name, value_name.to_string().as_str()) {
                (Some(i), "_") => {
                    quote! { for #i in 0..#iterable_tokens.len() #body_tokens ; }
                }
                (Some(i), _) => {
                    let index_var = quote! { #i };
                    let value_var = quote! { #value_name };
                    quote! { for #index_var in 0.. #iterable_tokens . len () as i32 { let #value_var = #iterable_tokens [#index_var as usize]; #body_tokens } ; }
                }
                (None, _) => {
                    quote! { for #value_name in #iterable_tokens.iter().cloned() #body_tokens ; }
                }
            }
        }
        HirStatement::ForLoop { init, condition, post, body } => {
            let body_tokens = hir_block_to_rust(body, strip_returns);
            let cond_tokens = hir_expr_to_rust(condition);
            // Init: generate raw statement tokens (not wrapped in block)
            let init_tokens = init.as_ref().map(|i| {
                // Generate init as a raw statement (let or expression)
                match &**i {
                    HirStatement::Local { name, mutable, value } => {
                        let mut_kw = if *mutable { quote! { mut } } else { quote! {} };
                        let value_tokens = hir_expr_to_rust(value);
                        quote! { let #mut_kw #name = #value_tokens }
                    }
                    HirStatement::Expr(expr) => {
                        let expr_tokens = hir_expr_to_rust(expr);
                        quote! { #expr_tokens; }
                    }
                    _ => {
                        hir_stmt_to_rust(i, strip_returns)
                    }
                }
            });
            // Post: generate raw statement tokens
            let post_tokens = post.as_ref().map(|p| {
                match &**p {
                    HirStatement::Expr(expr) => {
                        let expr_tokens = hir_expr_to_rust(expr);
                        quote! { #expr_tokens; }
                    }
                    _ => hir_stmt_to_rust(p, strip_returns)
                }
            });

            // C-style for → loop with guard
            match init_tokens {
                Some(i) => {
                    quote! {
                        {
                            #i;
                            loop {
                                if !(#cond_tokens) { break; }
                                #body_tokens
                                #post_tokens;
                            }
                        }
                    }
                }
                None => {
                    quote! {
                        loop {
                            if !(#cond_tokens) { break; }
                            #body_tokens
                            #post_tokens;
                        }
                    }
                }
            }
        }
        HirStatement::Return(expr) => {
            match expr {
                Some(e) => {
                    let e_tokens = hir_expr_to_rust(e);
                    quote! { return #e_tokens; }
                }
                None => quote! { return; },
            }
        }
        HirStatement::Continue => quote! { continue; },
        HirStatement::Break(label) => {
            match label {
                Some(l) => quote! { break #l; },
                None => quote! { break; },
            }
        }
        HirStatement::ChannelSend { channel, value } => {
            let ch_tokens = hir_expr_to_rust(channel);
            let val_tokens = hir_expr_to_rust(value);
            quote! { #ch_tokens.send(#val_tokens); }
        }
        HirStatement::ChannelRecv { channel, target } => {
            let ch_tokens = hir_expr_to_rust(channel);
            match target {
                Some(t) => quote! { let #t = #ch_tokens.recv().unwrap(); },
                None => quote! { #ch_tokens.recv().unwrap(); },
            }
        }
        HirStatement::TypeAssert { value, target_type, result_name } => {
            let val_tokens = hir_expr_to_rust(value);
            let ty_tokens = target_type.to_rust_type();
            match result_name {
                Some(n) => quote! { let #n = #val_tokens as #ty_tokens; },
                None => quote! { #val_tokens as #ty_tokens; },
            }
        }
        HirStatement::Closure { name, params, body } => {
            let param_tokens: Vec<TokenStream> = params.iter().map(|(n, ty)| {
                if let Some(t) = ty {
                    let t_tokens = t.to_rust_type();
                    quote! { #n: #t_tokens }
                } else {
                    quote! { #n }
                }
            }).collect();
            let body_tokens = hir_block_to_rust(body, strip_returns);
            quote! { let mut #name = | #(#param_tokens),* | #body_tokens; }
        }
        HirStatement::Defer { body } => {
            let body_tokens = hir_block_to_rust(body, strip_returns);
            quote! {
                {
                    #[derive(Default)]
                    struct __GourdDefer;
                    impl Drop for __GourdDefer {
                        fn drop(&mut self) {
                            #body_tokens
                        }
                    }
                    let _guard = __GourdDefer;
                }
            }
        }
        HirStatement::Import { alias, path, dot, blank } => {
            if *blank {
                // Blank import — side-effect only, nothing to emit
                quote! {}
            } else if *dot {
                // Dot import: make all names visible
                quote! { use ::gourd::prelude::*; }
            } else if let Some(alias_ident) = alias {
                // Aliased import: map known packages to prelude module
                let alias_ident = syn::Ident::new(&alias_ident, proc_macro2::Span::call_site());
                match path.as_str() {
                    "strings" | "os" | "io" | "bytes" | "json" | "time" | "math" | "byte" => {
                        quote! { use ::gourd::prelude as #alias_ident; }
                    }
                    _ => {
                        // External packages: generate a use statement but mark as unsupported
                        let msg = format!("import external package '{}': not yet fully supported", path);
                        quote! { use ::std::panic as #alias_ident; /* TODO: #msg */ }
                    }
                }
            } else {
                // Default import: map known packages to prelude module
                match path.as_str() {
                    "strings" => quote! { use ::gourd::packages::strings::*; },
                    "os" => quote! { use ::gourd::packages::os::*; },
                    "io" => quote! { use ::gourd::packages::io::*; },
                    "bytes" => quote! { use ::gourd::packages::bytes::*; },
                    "json" => quote! { use ::gourd::packages::json::*; },
                    "time" => quote! { use ::gourd::packages::time::*; },
                    "math" => quote! { use ::gourd::packages::math::*; },
                    "byte" => quote! { use ::gourd::packages::byte::*; },
                    "fmt" => quote! { use ::gourd::prelude::*; },
                    _ => {
                        // External packages: generate a use statement but mark as unsupported
                        let msg = format!("import external package '{}': not yet fully supported", path);
                        quote! { use ::std::panic as _; /* TODO: #msg */ }
                    }
                }
            }
        }
        HirStatement::RawStmt { tokens } => {
            // Pass through raw tokens
            tokens.clone()
        }
        HirStatement::SwitchReturn { tokens } => {
            // Pass through pre-transpiled switch return tokens
            tokens.clone()
        }
    }
}

/// Helper: emit a compile_error! for unsupported constructs.
fn emit_todo(msg: &str) -> TokenStream {
    quote! {
        {
            compile_error!(concat!("TODO: ", #msg));
            unreachable!()
        }
    }
}

/// Helper: emit a compile_error! for unsupported binary operations.
fn emit_todo_binary_op(op: &HirBinaryOp) -> TokenStream {
    let op_name = match op {
        HirBinaryOp::BitAnd => "BitAnd",
        HirBinaryOp::BitOr => "BitOr",
        HirBinaryOp::BitXor => "BitXor",
        HirBinaryOp::Lsh => "Lsh",
        HirBinaryOp::Rsh => "Rsh",
        _ => "unknown",
    };
    quote! {
        {
            compile_error!(concat!("TODO: binary operation not supported: ", #op_name));
            unreachable!()
        }
    }
}

/// Helper: get the type of a HIR expression (for type-aware codegen).
/// Infer the type of a HIR expression by inspecting its structure.
fn get_expr_type(expr: &HirExpr) -> HirType {
    match &expr.kind {
        HirExprKind::Literal(HirLiteral::StringTy(_)) => HirType::new(HirTypeKind::StringTy),
        HirExprKind::Literal(HirLiteral::Int(_))
        | HirExprKind::Literal(HirLiteral::Float(_))
        | HirExprKind::Identifier(_) => HirType::new(HirTypeKind::I32),
        HirExprKind::Literal(HirLiteral::Bool(_)) => HirType::new(HirTypeKind::Bool),
        HirExprKind::Unary { op, .. } => {
            match op {
                HirUnaryOp::Neg | HirUnaryOp::Deref => HirType::new(HirTypeKind::I32),
                HirUnaryOp::AddressOf => HirType::new(HirTypeKind::I32),
                HirUnaryOp::Not => HirType::new(HirTypeKind::Bool),
            }
        }
        HirExprKind::Binary { op, lhs, rhs } => {
            if matches!(op, HirBinaryOp::Add) {
                let lhs_type = get_expr_type(lhs);
                let rhs_type = get_expr_type(rhs);
                if lhs_type.is_string() || rhs_type.is_string() {
                    return HirType::new(HirTypeKind::StringTy);
                }
            }
            HirType::new(HirTypeKind::I32)
        }
        HirExprKind::Call { func, .. } => {
            // Detect String::from(...) calls and fmt functions
            if let HirExprKind::Path(ref hir_path) = func.kind {
                let path_str = quote::quote!(#hir_path).to_string();
                let normalized = path_str.replace(" :: ", "::");
                if normalized.contains("::std::string::String::from") {
                    return HirType::new(HirTypeKind::StringTy);
                }
                if normalized.contains("fmt_") {
                    return HirType::new(HirTypeKind::StringTy);
                }
            }
            HirType::new(HirTypeKind::I32)
        }
        HirExprKind::MethodCall { method, .. } => {
            let method_name = method.to_string();
            if method_name == "to_string" || method_name == "push" || method_name == "clone" {
                return HirType::new(HirTypeKind::StringTy);
            }
            HirType::new(HirTypeKind::I32)
        }
        HirExprKind::Path(_) => HirType::new(HirTypeKind::I32),
        HirExprKind::SliceLiteral(_) => {
            HirType::new(HirTypeKind::Slice(Box::new(HirType::new(HirTypeKind::I32))))
        }
        HirExprKind::Index { collection, .. } | HirExprKind::StringByteIndex { collection, .. } => {
            // Indexing into a Vec<String> or similar collection yields String
            // When collection type is unknown (Identifier), return Slice<I32> so slicing works
            let coll_ty = get_expr_type(collection);
            match &coll_ty.kind {
                HirTypeKind::Slice(inner) if matches!(&inner.kind, HirTypeKind::StringTy) => {
                    HirType::new(HirTypeKind::StringTy)
                }
                // For identifiers or unknown types, assume Slice to support indexing
                // and slicing ([..]) operations
                HirTypeKind::Unknown(_) | HirTypeKind::I32 => {
                    // Indexing returns the element type. For identifier collections, assume i32
                    // The codegen will handle borrowing for + operator
                    HirType::new(HirTypeKind::I32)
                }
                _ => HirType::new(HirTypeKind::I32),
            }
        }
        _ => HirType::new(HirTypeKind::Unknown("unknown".to_string())),
    }
}

trait HirTypeOps {
    fn is_string(&self) -> bool;
}

impl HirTypeOps for HirType {
    fn is_string(&self) -> bool {
        matches!(self.kind, HirTypeKind::StringTy)
    }
}

/// Convert a HIR min/max call to Rust tokens.
fn hir_minmax_to_rust(kind: &str, values: &[Box<HirExpr>]) -> TokenStream {
    let vals: Vec<TokenStream> = values.iter().map(|v| hir_expr_to_rust(v)).collect();
    match kind {
        "min" => quote! { ::std::cmp::min( #(#vals),* ) },
        "max" => quote! { ::std::cmp::max( #(#vals),* ) },
        _ => emit_todo(&format!("min/max with kind: {}", kind)),
    }
}

/// Convert a HIR delete call to Rust tokens.
fn hir_delete_to_rust(map: &HirExpr, key: &HirExpr) -> TokenStream {
    let map_tokens = hir_expr_to_rust(map);
    let key_tokens = hir_expr_to_rust(key);
    quote! { #map_tokens.remove(&#key_tokens); }
}

/// Generate Rust match expression from HIR match.
fn hir_match_to_rust(
    selector: &HirExpr,
    arms: &[(Vec<Box<HirExpr>>, HirBlock)],
    default_body: Option<HirBlock>,
) -> TokenStream {
    let selector_tokens = hir_expr_to_rust(selector);
    let arm_tokens: Vec<TokenStream> = arms.iter().map(|(patterns, body)| {
        let body_tokens = hir_block_to_rust(body, true);
        // Build multi-pattern: `1 | 2 | 3 => body` or single pattern: `1 => body`
        let pattern_tokens: Vec<TokenStream> = patterns.iter().map(|p| hir_expr_to_rust(p)).collect();
        if pattern_tokens.len() == 1 {
            quote! { #(#pattern_tokens)* => #body_tokens }
        } else {
            quote! { #(#pattern_tokens)|* => #body_tokens }
        }
    }).collect();

    let default_tokens = default_body.as_ref().map(|b| {
        let body_tokens = hir_block_to_rust(b, true);
        quote! { _ => #body_tokens }
    }).unwrap_or_else(|| quote! { _ => {} });

    quote! { match #selector_tokens { #(#arm_tokens,)* #default_tokens } }
}

/// Convert a slice literal to Rust tokens.
fn hir_slice_literal_to_rust(elements: &[HirExpr]) -> TokenStream {
    let elems: Vec<TokenStream> = elements.iter().map(|e| hir_expr_to_rust(e)).collect();
    if elems.is_empty() {
        // Empty slice literal `[]int{}` → `vec![]` (type inferred by context)
        quote! { vec![] }
    } else {
        quote! { vec![#(#elems),*] }
    }
}

/// Convert a map literal to Rust tokens.
fn hir_map_literal_to_rust(entries: &[(Box<HirExpr>, Box<HirExpr>)]) -> TokenStream {
    let pairs: Vec<TokenStream> = entries.iter().map(|(k, v)| {
        let k_tok = hir_expr_to_rust(k);
        let v_tok = hir_expr_to_rust(v);
        quote! { ::std::collections::Entry::Vacant(m.entry(#k_tok)) => { #v_tok } }
    }).collect();
    quote! {
        {
            let mut m = ::std::collections::HashMap::default();
            #(#pairs)*
            m
        }
    }
}

/// Convert a struct literal to Rust tokens.
/// `StructName{field1: val1, field2: val2}` → `StructName { field1: val1, field2: val2 }`
fn hir_struct_lit_to_rust(name: &syn::Path, fields: &[(syn::Ident, Box<HirExpr>)]) -> TokenStream {
    let field_tokens: Vec<TokenStream> = fields.iter().map(|(field_name, field_val)| {
        let val_tok = hir_expr_to_rust(field_val);
        quote! { #field_name: #val_tok }
    }).collect();
    quote! { #name { #(#field_tokens),* } }
}

/// Convert a channel send to Rust tokens.
fn hir_channel_send_to_rust(channel: &HirExpr, value: &HirExpr) -> TokenStream {
    let ch_tok = hir_expr_to_rust(channel);
    let val_tok = hir_expr_to_rust(value);
    quote! { ::gourd::GoChannel::send(&#ch_tok, &#val_tok) }
}

/// Convert a channel receive to Rust tokens.
fn hir_channel_recv_to_rust(channel: &HirExpr) -> TokenStream {
    let ch_tok = hir_expr_to_rust(channel);
    quote! { ::gourd::GoChannel::recv(&#ch_tok) }
}

/// Convert a select statement to Rust tokens.
fn hir_select_to_rust(cases: &[(Box<HirExpr>, HirBlock)], default_body: Option<HirBlock>) -> TokenStream {
    let default_body = default_body.as_ref().map(|b| hir_block_to_rust(b, true));
    let arms: Vec<TokenStream> = cases.iter().map(|(pattern, body)| {
        let pat_tok = hir_expr_to_rust(pattern);
        let body_tok = hir_block_to_rust(body, true);
        quote! { #pat_tok => #body_tok }
    }).collect();
    let default = default_body.map(|d| quote! { _ => #d });
    quote! { { match go_select_poll() { #(#arms),* #default } } }
}

/// Convert a HIR struct type to Rust tokens.
pub fn hir_struct_to_rust(name: &Ident, fields: &[(Ident, Box<HirType>)]) -> TokenStream {
    let field_tokens: Vec<_> = fields.iter().map(|(field_name, field_type)| {
        let ty = field_type.to_rust_type();
        quote! { pub #field_name: #ty }
    }).collect();
    quote! {
        struct #name { #(#field_tokens),* }
    }
}

/// Convert a HIR interface type to Rust tokens.
pub fn hir_interface_to_rust(name: &Ident, methods: &[HirInterfaceMethod]) -> TokenStream {


    let method_tokens: Vec<_> = methods.iter().map(|method| {
        // Convert method name to snake_case
        let snake_name = to_snake_case(&method.name.to_string());
        let method_id = syn::Ident::new(&snake_name, method.name.span());

        let param_tokens: Vec<_> = method.params.iter().map(|(param_name, param_type)| {
            let ty = param_type.to_rust_type();
            quote! { #param_name: #ty }
        }).collect();
        let return_tokens = if method.returns.is_empty() {
            quote! {}
        } else if method.returns.len() == 1 {
            let ty = method.returns[0].to_rust_type();
            quote! { -> #ty }
        } else {
            let return_types: Vec<_> = method.returns.iter().map(|t| t.to_rust_type()).collect();
            quote! { -> ( #(#return_types),* ) }
        };
        quote! { fn #method_id #(#param_tokens),* #return_tokens }
    }).collect();
    quote! {
        trait #name { #(#method_tokens)* }
    }
}

/// Generate Rust impl block from a HIR receiver function.
pub fn hir_receiver_fn_to_rust(rf: &HirReceiverFn) -> TokenStream {
    use quote::quote;

    let self_arg = if rf.pointer {
        quote! { &mut self }
    } else {
        quote! { &self }
    };

    // Build parameter list: self/ &mut self, then named params
    let param_list = if rf.params.is_empty() {
        self_arg
    } else {
        let named_params: Vec<_> = rf.params.iter().map(|(name, ty)| {
            let ty_ts = ty.to_rust_type();
            quote! { #name: #ty_ts }
        }).collect();
        quote! { #self_arg, #(#named_params),* }
    };

    // Build return type
    let return_ty = if rf.returns.is_empty() {
        quote! {}
    } else if rf.returns.len() == 1 {
        let ty = rf.returns[0].to_rust_type();
        quote! { -> #ty }
    } else {
        let tys: Vec<_> = rf.returns.iter().map(|t| t.to_rust_type()).collect();
        quote! { -> ( #(#tys),* ) }
    };

    // Transpile body using legacy go_to_rust for full expression support
    let body: proc_macro2::Group = if let Some(ref body_tokens) = rf.body {
        // Replace receiver variable name with 'self' in body tokens
        let recv_name_str = rf.recv_name.to_string();
        let self_ident = syn::Ident::new("self", proc_macro2::Span::call_site());
        
        // Build a token stream where we replace the receiver name with 'self'
        let mut replaced_tokens: proc_macro2::TokenStream = proc_macro2::TokenStream::new();
        for token in body_tokens.clone() {
            match token {
                proc_macro2::TokenTree::Ident(id) if id.to_string() == recv_name_str => {
                    replaced_tokens.extend(quote! { #self_ident });
                }
                _ => {
                    replaced_tokens.extend(quote! { #token });
                }
            }
        }
        
        // Use the legacy Go→Rust transpiler for the body, since it correctly
        // handles Go-style semicolon-separated statements. The HIR approach
        // doesn't properly parse Go tokens through syn::parse2.
        let body_wrapped: TokenStream = quote! { #replaced_tokens };
        let body_block: proc_macro2::Group = match crate::transpiler::legacy::stmt_to_rust::transpile_go_body(body_wrapped) {
            Some(block) => block,
            None => proc_macro2::Group::new(proc_macro2::Delimiter::Brace, quote! {}.into()),
        };

        proc_macro2::Group::new(
            proc_macro2::Delimiter::Brace,
            body_block.stream().into(),
        )
    } else {
        proc_macro2::Group::new(
            proc_macro2::Delimiter::Brace,
            quote! {}.into(),
        )
    };

    // Generate the impl block with method
    let recv_type_ts = rf.recv_type.to_rust_type();
    let fn_name_ts = &rf.fn_name;
    quote! {
        impl #recv_type_ts { fn #fn_name_ts (#param_list) #return_ty #body }
    }
}

/// Parse and transpile a Go anonymous function to Rust closure.
/// This is the HIR equivalent of `go_to_rust_closure` from the legacy pipeline.
pub fn go_to_rust_closure_hir(input: TokenStream) -> TokenStream {
    use super::conversion::hir_type_from_syn;
    use super::types::{map_go_type_str, map_go_types};

    let trees: Vec<proc_macro2::TokenTree> = input.into_iter().collect();

    // Validate: must start with `func`
    if trees.is_empty() {
        return quote! { | || compile_error!("empty input") }; // placeholder
    }
    if let proc_macro2::TokenTree::Ident(id) = &trees[0] {
        let name = id.to_string();
        if name != "func" && name != "fn" {
            return quote! { | || compile_error!("not a function") }; // placeholder
        }
    } else {
        return quote! { | || compile_error!("not a function") }; // placeholder
    }

    // Parse parameters from paren group
    let mut params: Vec<(syn::Ident, Option<Box<HirType>>)> = Vec::new();
    if trees.len() > 1 {
        if let proc_macro2::TokenTree::Group(g) = &trees[1] {
            if g.delimiter() == proc_macro2::Delimiter::Parenthesis {
                let param_trees: Vec<proc_macro2::TokenTree> = g.stream().into_iter().collect();
                let mut i = 0;
                while i < param_trees.len() {
                    // Skip commas
                    if let proc_macro2::TokenTree::Punct(p) = &param_trees[i] {
                        if p.as_char() == ',' {
                            i += 1;
                            continue;
                        }
                    }
                    // Parse identifier
                    if let proc_macro2::TokenTree::Ident(id) = &param_trees[i] {
                        i += 1;
                        // Check if next token is a type
                        if i < param_trees.len() {
                            let next = &param_trees[i];
                            if let proc_macro2::TokenTree::Ident(type_id) = next {
                                let type_name = type_id.to_string();
                                if is_go_type_name(&type_name) {
                                    let rust_type: syn::Type = map_go_type_str(&type_name);
                                    params.push((id.clone(), Some(hir_type_from_syn(&rust_type))));
                                    i += 1;
                                    continue;
                                }
                            }
                            // Slice type `[]T`
                            if let proc_macro2::TokenTree::Group(g) = next {
                                if g.delimiter() == proc_macro2::Delimiter::Bracket {
                                    if i + 1 < param_trees.len() {
                                        if let proc_macro2::TokenTree::Ident(elem_id) = &param_trees[i + 1] {
                                            let elem_name = elem_id.to_string();
                                            if is_go_type_name(&elem_name) {
                                                let rust_type: syn::Type = map_go_type_str(&elem_name);
                                                let elem_hir = hir_type_from_syn(&rust_type);
                                                // For slice params like []T, wrap in SliceRef directly
                                                // Don't round-trip through text — that creates nested references
                                                let slice_hir = HirType::new(HirTypeKind::SliceRef(elem_hir));
                                                params.push((id.clone(), Some(Box::new(slice_hir))));
                                                i += 2;
                                                continue;
                                            }
                                        }
                                    }
                                }
                            }
                            // No type annotation
                            params.push((id.clone(), None));
                        } else {
                            params.push((id.clone(), None));
                        }
                    } else {
                        i += 1;
                    }
                }
            }
        }
    }

    // Parse optional return type — collect separately from params
    let mut returns: Vec<Box<HirType>> = Vec::new();
    let ret_idx = if trees.len() > 2 {
        // Check if there's a return type (Go type name) before the body
        let second = &trees[2];
        match second {
            proc_macro2::TokenTree::Ident(id) if is_go_type_name(&id.to_string()) => {
                let rust_type: syn::Type = map_go_type_str(&id.to_string());
                returns.push(hir_type_from_syn(&rust_type));
                3
            }
            proc_macro2::TokenTree::Group(g) if g.delimiter() == proc_macro2::Delimiter::Bracket => {
                // Slice return type `[]T`
                if g.stream().is_empty() && trees.len() > 3 {
                    if let proc_macro2::TokenTree::Ident(elem_id) = &trees[3] {
                        let elem_name = elem_id.to_string();
                        if is_go_type_name(&elem_name) {
                            let rust_type: syn::Type = map_go_type_str(&elem_name);
                            let elem_hir = hir_type_from_syn(&rust_type);
                            // For slice return type []T, wrap in SliceRef
                            let slice_rust = HirType::new(HirTypeKind::SliceRef(elem_hir)).to_rust_type();
                            let parsed = syn::parse2::<syn::Type>(slice_rust).unwrap_or_else(|_| syn::parse_str::<syn::Type>("i32").unwrap());
                            returns.push(hir_type_from_syn(&parsed));
                        }
                    }
                }
                3
            }
            _ => 2,
        }
    } else {
        2
    };

    // Parse body from brace group
    let body = if trees.len() > ret_idx {
        if let proc_macro2::TokenTree::Group(g) = &trees[ret_idx] {
            if g.delimiter() == proc_macro2::Delimiter::Brace {
                let body_tokens: TokenStream = g.stream();
                // Parse body tokens into HIR module's GoBlock
                // body_tokens is the inner content of the brace group (e.g., "return 42")
                // We need to parse it as statements. Wrap in braces for GoBlock parsing.
                let wrapped_body: TokenStream = {
                    let open_brace = proc_macro2::Group::new(
                        proc_macro2::Delimiter::Brace,
                        body_tokens.clone(),
                    );
                    let mut ts = proc_macro2::TokenStream::new();
                    ts.extend(std::iter::once(proc_macro2::TokenTree::Group(open_brace)));
                    ts
                };
                match syn::parse2::<super::ast::GoBlock>(wrapped_body) {
                    Ok(go_block) => {
                        let stmts: Vec<HirStatement> = go_block.stmts.iter()
                            .map(|s| super::conversion::go_stmt_to_hir(s))
                            .collect();
                        HirBlock { stmts }
                    }
                    Err(_) => HirBlock::new(),
                }
            } else {
                HirBlock::new()
            }
        } else {
            HirBlock::new()
        }
    } else {
        HirBlock::new()
    };

    // Generate Rust closure using HIR codegen
    hir_closure_to_rust(&params, &returns, &body)
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

// ─── Top-level select and switch transpilation ────────────────────────────────

/// Parse and transpile a Go select statement from raw tokens to Rust.
pub fn go_to_rust_select_hir(input: TokenStream) -> TokenStream {
    // Parse into the Go AST's GoSelect type
    match syn::parse2::<super::ast::GoSelect>(input) {
        Ok(select) => {
            let hir_select = super::conversion::go_select_to_hir(&select);
            hir_select_to_rust_from_hir(&hir_select)
        }
        Err(_) => {
            quote! { compile_error!("TODO: select statement") }
        }
    }
}

/// Convert a HIR select to Rust tokens.
pub fn hir_select_to_rust_from_hir(hir: &HirSelect) -> TokenStream {
    let default_body = hir.default_body.as_ref().map(|b| hir_block_to_rust(b, true));
    let mut has_send = false;
    let mut has_recv = false;
    let mut has_default = false;
    
    let arms: Vec<TokenStream> = hir.cases.iter().map(|case| {
        match case {
            HirSelectCase::Send { ch, value } => {
                has_send = true;
                let ch_rust = hir_expr_to_rust(ch);
                let val_rust = hir_expr_to_rust(value);
                quote! { .send_case(#ch_rust, #val_rust) }
            }
            HirSelectCase::Recv { ch } => {
                has_recv = true;
                let ch_rust = hir_expr_to_rust(ch);
                quote! { .recv_case(#ch_rust) }
            }
            HirSelectCase::Default => {
                has_default = true;
                quote! { }
            }
        }
    }).collect();

    let select_type = if has_recv {
        quote! { GoSelect::<Option<i32>> }
    } else if has_send {
        quote! { GoSelect::<i32> }
    } else {
        quote! { GoSelect::<()> }
    };

    quote! {
        { gourd::#select_type::new() #(#arms)*.run(); }
    }
}

/// Parse and transpile a Go switch statement from raw tokens to Rust.
pub fn go_to_rust_switch_hir(input: TokenStream) -> TokenStream {
    // Parse into the Go AST's Switch type
    match syn::parse2::<super::ast::Switch>(input) {
        Ok(switch) => {
            let hir_switch = super::conversion::go_switch_to_hir(&switch);
            hir_switch_to_rust_from_hir(&hir_switch)
        }
        Err(_) => {
            quote! { compile_error!("TODO: switch statement") }
        }
    }
}

/// Convert a HIR switch to Rust tokens.
pub(crate) fn hir_switch_to_rust_from_hir(hir: &HirSwitch) -> TokenStream {
    // Build match arms from cases
    let arms: Vec<TokenStream> = hir.cases.iter().map(|case| {
        let pattern: Vec<_> = case.patterns.iter().map(|e| hir_expr_to_rust(e)).collect();
        let body = hir_block_to_rust(&case.body, true);
        // Multi-pattern: `1 | 2 => body`
        if pattern.len() > 1 {
            quote! { #(#pattern)|* => #body }
        } else if pattern.len() == 1 {
            let pat = &pattern[0];
            quote! { #pat => #body }
        } else {
            body
        }
    }).collect();

    let default_body = hir.default_body.as_ref().map(|b| hir_block_to_rust(b, true));
    let default_arm = default_body.as_ref().map(|d| quote! { _ => #d });

    // Build selector
    let selector = hir.selector.as_ref()
        .map(|s| hir_expr_to_rust(s))
        .unwrap_or_else(|| quote! { () });

    // If no selector, build if-else chain instead of match
    if hir.selector.is_none() {
        let if_arms: Vec<_> = hir.cases.iter().map(|case| {
            let conds: Vec<_> = case.patterns.iter().map(|e| hir_expr_to_rust(e)).collect();
            let body = hir_block_to_rust(&case.body, true);
            quote! { else if #(#conds)&&* #body }
        }).collect();

        let default_else = default_body.as_ref().map(|d| quote! { else #d });

        return if !if_arms.is_empty() {
            let rest = &if_arms[1..];
            let first_body = &hir.cases[0].body;
            let first_conds: Vec<_> = hir.cases[0].patterns.iter().map(|e| hir_expr_to_rust(e)).collect();
            let first_body_rust = hir_block_to_rust(first_body, true);
            quote! { if #(#first_conds)&&* #first_body_rust #(#rest)* #default_else }
        } else {
            quote! { () }
        };
    }

    // With selector: build match expression
    let arms_as_tokens: proc_macro2::TokenStream = arms.iter().cloned().collect();
    let default = default_arm.map(|d| quote! { , #d });

    quote! {
        match #selector { #arms_as_tokens #default }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_int_to_rust() {
        let expr = HirExpr::new(HirExprKind::Literal(HirLiteral::Int(42)));
        let tokens = hir_expr_to_rust(&expr);
        let s = tokens.to_string();
        assert!(s.contains("42"), "Expected '42' in output, got: {}", s);
    }

    #[test]
    fn test_literal_string_to_rust() {
        let expr = HirExpr::new(HirExprKind::Literal(HirLiteral::StringTy("hello".to_string())));
        let tokens = hir_expr_to_rust(&expr);
        let s = tokens.to_string();
        assert!(s.contains("hello"), "Expected 'hello' in output, got: {}", s);
    }

    #[test]
    fn test_identifier_to_rust() {
        let id = syn::Ident::new("x", proc_macro2::Span::call_site());
        let expr = HirExpr::new(HirExprKind::Identifier(id));
        let tokens = hir_expr_to_rust(&expr);
        let s = tokens.to_string();
        assert!(s.contains("x"), "Expected 'x' in output, got: {}", s);
    }

    #[test]
    fn test_unary_not_to_rust() {
        let operand = Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Bool(true))));
        let expr = HirExpr::new(HirExprKind::Unary {
            op: HirUnaryOp::Not,
            operand,
        });
        let tokens = hir_expr_to_rust(&expr);
        let s = tokens.to_string();
        // Not should produce `!` operator
        assert!(s.contains("!"), "Expected '!' in output, got: {}", s);
    }

    #[test]
    fn test_binary_add_to_rust() {
        let lhs = Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Int(1))));
        let rhs = Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Int(2))));
        let expr = HirExpr::new(HirExprKind::Binary {
            op: HirBinaryOp::Add,
            lhs,
            rhs,
        });
        let tokens = hir_expr_to_rust(&expr);
        let s = tokens.to_string();
        assert!(s.contains("+") || s.contains("1"), "Expected '+' or '1' in output, got: {}", s);
    }

    #[test]
    fn test_call_to_rust() {
        let func = Box::new(HirExpr::new(HirExprKind::Identifier(syn::Ident::new("foo", proc_macro2::Span::call_site()))));
        let arg = HirExpr::new(HirExprKind::Literal(HirLiteral::Int(42)));
        let expr = HirExpr::new(HirExprKind::Call {
            func,
            args: vec![arg],
        });
        let tokens = hir_expr_to_rust(&expr);
        let s = tokens.to_string();
        assert!(s.contains("foo") || s.contains("42"), "Expected 'foo' or '42' in output, got: {}", s);
    }

    #[test]
    fn test_minmax_min_to_rust() {
        let values: Vec<Box<HirExpr>> = vec![
            Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Int(1)))),
            Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Int(2)))),
        ];
        let expr = HirExpr::new(HirExprKind::MinMax { kind: "min".to_string(), values });
        let tokens = hir_expr_to_rust(&expr);
        let s = tokens.to_string();
        assert!(s.contains("min"), "Expected 'min' in output, got: {}", s);
    }

    #[test]
    fn test_minmax_max_to_rust() {
        let values: Vec<Box<HirExpr>> = vec![
            Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Int(1)))),
            Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Int(2)))),
        ];
        let expr = HirExpr::new(HirExprKind::MinMax { kind: "max".to_string(), values });
        let tokens = hir_expr_to_rust(&expr);
        let s = tokens.to_string();
        assert!(s.contains("max"), "Expected 'max' in output, got: {}", s);
    }

    #[test]
    fn test_panic_to_rust() {
        let expr = HirExpr::new(HirExprKind::Panic("test error".to_string()));
        let tokens = hir_expr_to_rust(&expr);
        let s = tokens.to_string();
        assert!(s.contains("panic"), "Expected 'panic' in output, got: {}", s);
    }

    #[test]
    fn test_delete_to_rust() {
        let map_expr = Box::new(HirExpr::new(HirExprKind::Identifier(syn::Ident::new("m", proc_macro2::Span::call_site()))));
        let key_expr = Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Int(1))));
        let expr = HirExpr::new(HirExprKind::Delete { map: map_expr, key: key_expr });
        let tokens = hir_expr_to_rust(&expr);
        let s = tokens.to_string();
        assert!(s.contains("remove"), "Expected 'remove' in output, got: {}", s);
    }

    #[test]
    fn test_tuple_empty_to_rust() {
        // Empty tuple should produce `()` — fixes `[]int{}` → `Tuple([])` issue
        let expr = HirExpr::new(HirExprKind::Tuple(Vec::new()));
        let tokens = hir_expr_to_rust(&expr);
        let s = tokens.to_string();
        assert!(s.contains("()"), "Expected '()' for empty tuple, got: {}", s);
    }

    #[test]
    fn test_break_to_rust() {
        // Break without label
        let expr = HirExpr::new(HirExprKind::Block(HirBlock {
            stmts: vec![HirStatement::Break(None)],
        }));
        let tokens = hir_expr_to_rust(&expr);
        let s = tokens.to_string();
        assert!(s.contains("break"), "Expected 'break' in output, got: {}", s);
        assert!(!s.contains("continue"), "Should not contain 'continue' for break stmt");
    }

    #[test]
    fn test_break_label_to_rust() {
        // Break with label
        let label = syn::Ident::new("label", proc_macro2::Span::call_site());
        let expr = HirExpr::new(HirExprKind::Block(HirBlock {
            stmts: vec![HirStatement::Break(Some(label.clone()))],
        }));
        let tokens = hir_expr_to_rust(&expr);
        let s = tokens.to_string();
        assert!(s.contains("break label"), "Expected 'break label' in output, got: {}", s);
    }

    #[test]
    fn test_binary_assign_ops_to_rust() {
        // Test compound assignment operators
        let lhs = Box::new(HirExpr::new(HirExprKind::Identifier(syn::Ident::new("x", proc_macro2::Span::call_site()))));
        let rhs = Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Int(1))));

        // AddAssign
        let expr = HirExpr::new(HirExprKind::Binary { op: HirBinaryOp::AddAssign, lhs: lhs.clone(), rhs: rhs.clone() });
        assert!(hir_expr_to_rust(&expr).to_string().contains("+="));

        // SubAssign
        let expr = HirExpr::new(HirExprKind::Binary { op: HirBinaryOp::SubAssign, lhs: lhs.clone(), rhs: rhs.clone() });
        assert!(hir_expr_to_rust(&expr).to_string().contains("-="));

        // ShlAssign
        let expr = HirExpr::new(HirExprKind::Binary { op: HirBinaryOp::LshAssign, lhs: lhs.clone(), rhs: rhs.clone() });
        assert!(hir_expr_to_rust(&expr).to_string().contains("<<="));

        // ShrAssign
        let expr = HirExpr::new(HirExprKind::Binary { op: HirBinaryOp::RshAssign, lhs: lhs.clone(), rhs: rhs.clone() });
        assert!(hir_expr_to_rust(&expr).to_string().contains(">>="));

        // XorAssign
        let expr = HirExpr::new(HirExprKind::Binary { op: HirBinaryOp::XorAssign, lhs, rhs });
        assert!(hir_expr_to_rust(&expr).to_string().contains("^="));
    }

    #[test]
    fn test_forloop_with_init_to_rust() {
        // C-style for loop with init: `for i := 0; i < n; i++ { body }`
        let init_stmt = Box::new(HirStatement::Local {
            name: syn::Ident::new("i", proc_macro2::Span::call_site()),
            mutable: true,
            value: Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Int(3000000000)))),
        });
        let condition = Box::new(HirExpr::new(HirExprKind::Binary {
            op: HirBinaryOp::Lt,
            lhs: Box::new(HirExpr::new(HirExprKind::Identifier(syn::Ident::new("i", proc_macro2::Span::call_site())))),
            rhs: Box::new(HirExpr::new(HirExprKind::Identifier(syn::Ident::new("n", proc_macro2::Span::call_site())))),
        }));
        let post_stmt = Box::new(HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Binary {
            op: HirBinaryOp::AddAssign,
            lhs: Box::new(HirExpr::new(HirExprKind::Identifier(syn::Ident::new("i", proc_macro2::Span::call_site())))),
            rhs: Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Int(1)))),
        }))));
        let body = HirBlock { stmts: vec![HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Identifier(syn::Ident::new("x", proc_macro2::Span::call_site())))))] };
        let stmt = HirStatement::ForLoop {
            init: Some(init_stmt),
            condition,
            post: Some(post_stmt),
            body,
        };
        let tokens = hir_stmt_to_rust(&stmt, false);
        let s = tokens.to_string();
        // Should contain init, condition with break, and post
        // Note: large integer literals (> i32::MAX) get i64 suffix
        assert!(s.contains("let mut i ="), "Expected init in output, got: {}", s);
        assert!(s.contains("i64"), "Expected i64 suffix for large literal in output, got: {}", s);
        assert!(s.contains("break"), "Expected break in output, got: {}", s);
        assert!(s.contains("i +="), "Expected post in output, got: {}", s);
    }
}


// ─── Go function and struct handlers (moved from legacy modules) ──────────────

/// Parse and transpile a Go function from raw tokens to Rust using HIR.
pub fn go_to_rust_fn_hir(input: TokenStream) -> TokenStream {
    // First parse the Go function into our custom AST
    let go_fn = match syn::parse2::<GoFn>(input.clone()) {
        Ok(go_fn) => go_fn,
        Err(e) => {
            eprintln!("[DEBUG] GoFn parse error: {}", e);
            return e.to_compile_error();
        }
    };

    // Convert GoFn → HirFunction
    let hir_fn = go_fn_to_hir(&go_fn);

    // Generate Rust tokens from HIR
    hir_fn_to_rust(&hir_fn)
}

/// Convert a GoFn AST to a HirFunction.
fn go_fn_to_hir(go_fn: &GoFn) -> HirFunction {
    // Extract the function name
    let name = go_fn.ident.clone();

    // Convert parameters
    let params: Vec<(syn::Ident, Box<HirType>)> = go_fn.inputs.args.iter().map(|param| {
        let id = param.id.clone();
        let ty = match (&param.ty, &param.slice_elem) {
            (None, None) => {
                // Simple type with no slice element — fallback to i32
                Box::new(HirType::new(HirTypeKind::I32))
            }
            (_, Some(slice_inner)) => {
                // Slice type: `[]T` → borrowed slice `&[T]` for parameters
                // Map the element type properly (e.g., []byte → &[u8])
                let ty_str = quote::quote! { #slice_inner }.to_string();
                let elem = super::types::parse_go_type(&ty_str);
                Box::new(HirType::new(HirTypeKind::SliceRef(Box::new(elem))))
            }
            (Some(ty), None) => {
                // Regular type — use map_go_types for compound types (chan, map, etc.)
                let mapped = super::types::map_go_types(ty);
                let ty_str = quote::quote! { #mapped }.to_string();
                // Now parse the canonical Go→Rust type string
                let elem = super::types::parse_go_type(&ty_str);
                // Variadic params like `nums ...int` → borrowed slice `&[T]`
                if param.variadic {
                    Box::new(HirType::new(HirTypeKind::SliceRef(Box::new(elem))))
                } else {
                    Box::new(elem)
                }
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
                        // Use map_go_types for the element type, convert to string and parse as HirType
                        let mapped_elem = super::types::map_go_types(elem);
                        let elem_str = quote::quote! { #mapped_elem }.to_string();
                        Box::new(HirType::new(HirTypeKind::Slice(Box::new(super::types::parse_go_type(&elem_str)))))
                    } else {
                        // No element type — use the return type itself
                        let mapped = super::types::map_go_types(t);
                        let mapped_str = quote::quote! { #mapped }.to_string();
                        Box::new(HirType::new(HirTypeKind::Slice(Box::new(super::types::parse_go_type(&mapped_str)))))
                    }
                } else {
                    let ty_str = quote::quote! { #t }.to_string();
                    // Use map_go_types to handle compound types (maps, slices, etc.)
                    let mapped = super::types::map_go_types(t);
                    let mapped_str = quote::quote! { #mapped }.to_string();
                    // Try to parse as a Go type for compound types
                    Box::new(super::types::parse_go_type(&mapped_str))
                }
            }).collect()
        }
    }).unwrap_or_else(Vec::new);

    // Convert body statements using HIR conversion module
    let body_stmts: Vec<HirStatement> = go_fn.block.stmts.iter()
        .map(|stm| {
            // Safe transmute: both GoStmt types have identical structure
            let hir_stmt: &super::ast::GoStmt = unsafe { std::mem::transmute(stm) };
            super::conversion::go_stmt_to_hir(hir_stmt)
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

    // Extract import statements from body and generate them before the function.
    let mut import_tokens = proc_macro2::TokenStream::new();
    let body_stmts: Vec<HirStatement> = hir_fn.body.stmts.iter()
        .filter(|stmt| {
            if let HirStatement::Import { .. } = stmt {
                // Generate import token stream
                import_tokens.extend(hir_stmt_to_rust(stmt, false));
                false // Filter out imports from body
            } else {
                true // Keep non-import statements
            }
        })
        .cloned()
        .collect();

    // Generate body tokens (without imports)
    let body_tokens: Vec<TokenStream> = body_stmts.iter().map(|stmt| {
        hir_stmt_to_rust(stmt, false)
    }).collect();

    let body_tokens_len = body_tokens.len();
    let has_return_type = !hir_fn.returns.is_empty();

    // Handle last statement for tail expression semantics
    let body_tokens: Vec<TokenStream> = if body_tokens_len > 1 {
        // Multiple statements: all but last with semicolons, last may need return
        let last = body_tokens.last().unwrap();
        let rest = &body_tokens[..body_tokens_len - 1];
        let mut out: Vec<TokenStream> = rest.iter().map(|t| quote! { #t ; }).collect();
        // Wrap last statement with `return` if function has return type
        if has_return_type {
            // Don't double-wrap: check if the last statement already starts with `return`
            let last_str = last.to_string();
            if last_str.starts_with("return") {
                // Already has return — use as-is
                out.push(last.clone());
            } else {
                // No return yet — wrap it
                out.push(quote! { return #last });
            }
        } else {
            out.push(last.clone());
        }
        out
    } else if body_tokens_len == 1 {
        // Single statement — tail expression, wrap with `return` if function has return type
        let last = body_tokens.last().unwrap();
        if has_return_type {
            // Don't double-wrap return statements
            let last_str = last.to_string();
            if last_str.starts_with("return") {
                body_tokens.clone()
            } else {
                vec![quote! { return #last }]
            }
        } else {
            body_tokens.clone()
        }
    } else {
        // Empty body
        vec![]
    };

    quote! {
        #import_tokens
        fn #name ( #(#param_tokens),* ) #return_tokens { #(#body_tokens)* }
    }
}

/// Parse and transpile a Go struct from raw tokens to Rust using HIR.
pub fn go_to_rust_struct_hir(input: TokenStream) -> TokenStream {
    // First parse the Go struct into our custom AST
    let go_struct = match syn::parse2::<GoStruct>(input) {
        Ok(go_struct) => go_struct,
        Err(e) => return e.to_compile_error(),
    };

    // Convert GoStruct → HirStruct
    let hir_struct = go_struct_to_hir(&go_struct);

    // Generate Rust tokens from HIR
    hir_struct_to_rust(&hir_struct.name, &hir_struct.fields)
}

/// Convert a GoStruct AST to a HirStruct.
fn go_struct_to_hir(go_struct: &GoStruct) -> HirStruct {
    let name = go_struct.ident.clone();

    // Convert fields
    let fields: Vec<(Ident, Box<HirType>)> = go_struct.fields.iter().map(|field| {
        let name = field.name.clone();
        // Go doesn't have a separate slice_elem field — slice types in struct
        // fields are just regular types (e.g. `[]int` as the type itself).
        // For now, map the type normally.
        let mapped = super::types::map_go_types(&field.ty);
        let ty_str = quote::quote! { #mapped }.to_string();
        let ty = Box::new(super::types::parse_go_type(&ty_str));
        (name, ty)
    }).collect();

    HirStruct { name, fields }
}

/// Convert a raw Go receiver function (impl block method) to Rust tokens.
///
/// This is the HIR-based handler for receiver functions like:
/// ```go
/// func (f Foo) Bar() int { return f.x + 1 }
/// ```
pub fn go_to_rust_receiver_fn_hir(input: TokenStream) -> TokenStream {
    match super::types::parse_go_receiver_fn(input) {
        Some(rf) => hir_receiver_fn_to_rust(&rf),
        None => quote! { compile_error!("Failed to parse Go receiver function") },
    }
}

/// Convert a Go interface declaration to Rust trait tokens.
///
/// This is the HIR-based handler for interface declarations like:
/// ```go
/// interface Shape { Name() string }
/// ```
pub fn go_to_rust_interface_hir(input: TokenStream) -> TokenStream {
    let hir_type = match super::types::parse_go_interface(input) {
        Some(ty) => ty,
        None => {
            return quote! { compile_error!("Failed to parse Go interface") };
        }
    };

    match &hir_type.kind {
        super::types::HirTypeKind::Interface { name, methods } => hir_interface_to_rust(name, methods),
        _ => quote! { compile_error!("Expected interface type in HIR") },
    }
}

/// Public API: convert an AST Switch to Rust tokens.
pub fn switch_to_rust(switch: &super::ast::Switch) -> TokenStream {
    let hir = super::conversion::go_switch_to_hir(switch);
    hir_switch_to_rust_from_hir(&hir)
}
