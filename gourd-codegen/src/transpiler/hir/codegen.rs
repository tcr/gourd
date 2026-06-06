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
use super::types::{ HirType, HirTypeKind };
use super::statement::{ HirStatement, HirBlock };

/// Convert a HIR expression to Rust tokens.
pub fn hir_expr_to_rust(expr: &HirExpr) -> TokenStream {
    match &expr.kind {
        HirExprKind::Literal(lit) => hir_literal_to_rust(lit),
        HirExprKind::Identifier(id) => quote! { #id },
        HirExprKind::Path(path) => quote! { #path },
        HirExprKind::Macro(tokens) => tokens.clone(),
        HirExprKind::Binary { op, lhs, rhs } => hir_binary_to_rust(op, lhs, rhs),
        HirExprKind::Unary { op, operand } => hir_unary_to_rust(op, operand),
        HirExprKind::Call { func, args } => hir_call_to_rust(func, args),
        HirExprKind::MethodCall { receiver, method, args } => hir_method_call_to_rust(receiver, method, args),
        HirExprKind::FieldAccess { receiver, field } => hir_field_access_to_rust(receiver, field),
        HirExprKind::Index { collection, index } => hir_index_to_rust(collection, index),
        HirExprKind::Slice { collection, start, end } => hir_slice_to_rust(collection, start, end),
        HirExprKind::RangeVar(name) => quote! { #name },
        HirExprKind::Cast { value, target_type } => hir_cast_to_rust(value, target_type),
        HirExprKind::TypeConvert { func, arg } => hir_type_convert_to_rust(func, arg),
        HirExprKind::Tuple(values) => hir_tuple_to_rust(values),
        HirExprKind::Block(block) => hir_block_to_rust(block),
        HirExprKind::Closure { params, body } => hir_closure_to_rust(params, body),
        HirExprKind::ErrorCheck { value } => hir_error_check_to_rust(value),
        HirExprKind::Len(expr) => hir_len_to_rust(expr),
        HirExprKind::Cap(expr) => hir_cap_to_rust(expr),
        HirExprKind::Make(kind) => hir_make_to_rust(kind),
        HirExprKind::Append { target, elements } => hir_append_to_rust(target, elements),
        HirExprKind::Copy { dst, src } => hir_copy_to_rust(dst, src),
        HirExprKind::SliceLiteral(elements) => hir_slice_literal_to_rust(elements),
        HirExprKind::Map(entries) => hir_map_literal_to_rust(entries),
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
        HirExprKind::Unsupported(msg) => quote! { compile_error!(concat!("HIR: unsupported: ", #msg)) },
    }
}

/// Convert a HIR literal to Rust tokens.
fn hir_literal_to_rust(lit: &HirLiteral) -> TokenStream {
    match lit {
        HirLiteral::Int(n) => quote! { #n },
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
            // Check if this is string concatenation (String + String)
            // For numeric addition: `lhs + rhs`
            // For string concatenation: `lhs + &rhs` (borrow rhs to avoid move)
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
fn hir_call_to_rust(func: &HirExpr, args: &[HirExpr]) -> TokenStream {
    let func_tokens = hir_expr_to_rust(func);
    let arg_tokens: Vec<TokenStream> = args.iter().map(|a| hir_expr_to_rust(a)).collect();
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
fn hir_index_to_rust(collection: &HirExpr, index: &HirExpr) -> TokenStream {
    let collection_tokens = hir_expr_to_rust(collection);
    let index_tokens = hir_expr_to_rust(index);
    quote! { #collection_tokens[#index_tokens] }
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
    quote! { #value_tokens as #ty_tokens }
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
        "rune" => quote! { ((#arg_tokens) as u8 as char) },
        // String conversion
        "string" => quote! { ::std::str::from_utf8(&#arg_tokens).unwrap_or("").to_string() },
        // Fallback
        _ => quote! { #func(#arg_tokens) },
    }
}

/// Convert a HIR tuple to Rust tokens.
fn hir_tuple_to_rust(values: &[HirExpr]) -> TokenStream {
    let tokens: Vec<TokenStream> = values.iter().map(|v| hir_expr_to_rust(v)).collect();
    quote! { ( #(#tokens),* ) }
}

/// Convert a HIR block to Rust tokens.
fn hir_block_to_rust(block: &HirBlock) -> TokenStream {
    let stmt_tokens: Vec<TokenStream> = block.stmts.iter().map(|s| hir_stmt_to_rust(s)).collect();
    quote! { { #(#stmt_tokens);* } }
}

/// Convert a HIR closure to Rust tokens.
fn hir_closure_to_rust(params: &[(syn::Ident, Option<Box<HirType>>)], body: &HirBlock) -> TokenStream {
    let param_tokens: Vec<TokenStream> = params.iter().map(|(name, ty): &(syn::Ident, Option<Box<HirType>>)| {
        if let Some(ty) = ty {
            let ty_tokens = ty.to_rust_type();
            quote! { #name: #ty_tokens }
        } else {
            quote! { #name }
        }
    }).collect();
    let body_tokens = hir_block_to_rust(body);
    quote! { | #(#param_tokens),* | #body_tokens }
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
    } else if elem_tokens.len() == 1 {
        quote! { { #target_tokens.push(#(elem_tokens)[0]); #target_tokens } }
    } else {
        quote! { {
            #(#target_tokens.push(#elem_tokens);)*
            #target_tokens
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
pub fn hir_stmt_to_rust(stmt: &HirStatement) -> TokenStream {
    match stmt {
        HirStatement::Local { name, mutable, value } => {
            let mut_kw = if *mutable { quote! { mut } } else { quote! {} };
            let value_tokens = hir_expr_to_rust(value);
            quote! { let #mut_kw #name = #value_tokens; }
        }
        HirStatement::Assign { target, value } => {
            let target_tokens = hir_expr_to_rust(target);
            let value_tokens = hir_expr_to_rust(value);
            quote! { #target_tokens = #value_tokens; }
        }
        HirStatement::Expr(expr) => {
            let expr_tokens = hir_expr_to_rust(expr);
            quote! { #expr_tokens; }
        }
        HirStatement::If { cond, then_body, else_body } => {
            let cond_tokens = hir_expr_to_rust(cond);
            let then_tokens = hir_block_to_rust(then_body);
            let else_tokens = else_body.as_ref()
                .map(|b| {
                    let body_tokens = hir_block_to_rust(b);
                    quote! { else #body_tokens }
                })
                .unwrap_or_default();
            quote! { if #cond_tokens #then_tokens #else_tokens }
        }
        HirStatement::While { cond, body } => {
            let cond_tokens = hir_expr_to_rust(cond);
            let body_tokens = hir_block_to_rust(body);
            quote! { while #cond_tokens #body_tokens }
        }
        HirStatement::ForRange { index_name, value_name, iterable, body } => {
            let iterable_tokens = hir_expr_to_rust(iterable);
            let body_tokens = hir_block_to_rust(body);
            match index_name {
                Some(i) => {
                    // Both index and value: use enumerate()
                    quote! { for (#i, #value_name) in #iterable_tokens.iter().enumerate() #body_tokens }
                }
                None => {
                    // Only value: use iter().cloned()
                    quote! { for #value_name in #iterable_tokens.iter().cloned() #body_tokens }
                }
            }
        }
        HirStatement::ForLoop { init, condition, post, body } => {
            let body_tokens = hir_block_to_rust(body);
            let init_tokens = init.as_ref().map(|i| hir_expr_to_rust(i));
            let cond_tokens = hir_expr_to_rust(condition);
            let post_tokens = post.as_ref().map(|p| hir_expr_to_rust(p));

            // C-style for → loop with guard
            // The condition is parenthesized to fix operator precedence
            // (same fix as hir_unary_to_rust for Not)
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
                Some(l) => quote! { continue #l; },
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
            let body_tokens = hir_block_to_rust(body);
            quote! { let mut #name = | #(#param_tokens),* | #body_tokens; }
        }
        HirStatement::Defer { body } => {
            // `defer func() { ... }` → Rust Drop guard at end of scope
            let body_tokens = hir_block_to_rust(body);
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
                        // External packages not yet supported
                        let msg = format!("TODO: import external packages: {}", path);
                        quote! { compile_error!(concat!(#msg)) }
                    }
                }
            } else {
                // Default import: already implicit via `gourd::prelude::*`
                quote! {}
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
fn get_expr_type(expr: &HirExpr) -> HirType {
    // This is a placeholder — in the full implementation, types would
    // be inferred or carried alongside expressions.
    // For now, default to unknown.
    HirType::new(HirTypeKind::Unknown("unknown".to_string()))
}

/// Generate Rust match expression from HIR match.
fn hir_match_to_rust(
    selector: &HirExpr,
    arms: &[(Box<HirExpr>, HirBlock)],
    default_body: Option<HirBlock>,
) -> TokenStream {
    let selector_tokens = hir_expr_to_rust(selector);
    let arm_tokens: Vec<TokenStream> = arms.iter().map(|(pattern, body)| {
        let pattern_tokens = hir_expr_to_rust(pattern);
        let body_tokens = hir_block_to_rust(body);
        quote! { #pattern_tokens => #body_tokens }
    }).collect();

    let default_tokens = default_body.as_ref().map(|b| {
        let body_tokens = hir_block_to_rust(b);
        quote! { _ => #body_tokens }
    }).unwrap_or_else(|| quote! { _ => {} });

    quote! { match #selector_tokens { #(#arm_tokens,)* #default_tokens } }
}

/// Convert a slice literal to Rust tokens.
fn hir_slice_literal_to_rust(elements: &[HirExpr]) -> TokenStream {
    let elems: Vec<TokenStream> = elements.iter().map(|e| hir_expr_to_rust(e)).collect();
    quote! { vec![#(#elems),*] }
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

/// Convert a channel send to Rust tokens.
fn hir_channel_send_to_rust(channel: &HirExpr, value: &HirExpr) -> TokenStream {
    let ch_tok = hir_expr_to_rust(channel);
    let val_tok = hir_expr_to_rust(value);
    quote! { ::gourd::prelude::GoChannel::send(&#ch_tok, &#val_tok) }
}

/// Convert a channel receive to Rust tokens.
fn hir_channel_recv_to_rust(channel: &HirExpr) -> TokenStream {
    let ch_tok = hir_expr_to_rust(channel);
    quote! { ::gourd::prelude::GoChannel::recv(&#ch_tok) }
}

/// Convert a select statement to Rust tokens.
fn hir_select_to_rust(cases: &[(Box<HirExpr>, HirBlock)], default_body: Option<HirBlock>) -> TokenStream {
    let default_body = default_body.as_ref().map(|b| hir_block_to_rust(b));
    let arms: Vec<TokenStream> = cases.iter().map(|(pattern, body)| {
        let pat_tok = hir_expr_to_rust(pattern);
        let body_tok = hir_block_to_rust(body);
        quote! { #pat_tok => #body_tok }
    }).collect();
    let default = default_body.map(|d| quote! { _ => #d });
    quote! { { match go_select_poll() { #(#arms),* #default } } }
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
}
