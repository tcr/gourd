//! Operator transpilation: `Binary`, `Unary`, `Cast`, `Assign`, `Break`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{BinOp, ExprAssign, ExprBreak, ExprCast, ExprContinue, ExprUnary, UnOp};

use super::dispatch::emit_todo;

pub fn transpile_binary(input: &syn::ExprBinary) -> TokenStream {
    let lhs = super::dispatch::go_to_rust(&input.left);
    let rhs = super::dispatch::go_to_rust(&input.right);
    match input.op {
        BinOp::Add(_)         => quote! { #lhs + #rhs },
        BinOp::Sub(_)         => quote! { #lhs - #rhs },
        BinOp::Mul(_)         => quote! { #lhs * #rhs },
        BinOp::Div(_)         => quote! { #lhs / #rhs },
        BinOp::Rem(_)         => quote! { #lhs % #rhs },
        BinOp::And(_)         => quote! { #lhs && #rhs },
        BinOp::Or(_)          => quote! { #lhs || #rhs },
        BinOp::BitXor(_)      => quote! { #lhs ^ #rhs },
        BinOp::BitAnd(_)      => quote! { #lhs & #rhs },
        BinOp::BitOr(_)       => quote! { #lhs | #rhs },
        BinOp::Shl(_)         => quote! { #lhs << #rhs },
        BinOp::Shr(_)         => quote! { #lhs >> #rhs },
        BinOp::AddAssign(_)   => quote! { #lhs += #rhs },
        BinOp::SubAssign(_)   => quote! { #lhs -= #rhs },
        BinOp::MulAssign(_)   => quote! { #lhs *= #rhs },
        BinOp::DivAssign(_)   => quote! { #lhs /= #rhs },
        BinOp::RemAssign(_)   => quote! { #lhs %= #rhs },
        BinOp::Eq(_)          => quote! { #lhs == #rhs },
        BinOp::Ne(_)          => quote! { #lhs != #rhs },
        BinOp::Ge(_)          => quote! { #lhs >= #rhs },
        BinOp::Gt(_)          => quote! { #lhs > #rhs },
        BinOp::Le(_)          => quote! { #lhs <= #rhs },
        BinOp::Lt(_)          => quote! { #lhs < #rhs },
        _                     => emit_todo("unsupported binary operator"),
    }
}

pub fn transpile_unary(input: &ExprUnary) -> TokenStream {
    let inner = super::dispatch::go_to_rust(&input.expr);
    match &input.op {
        UnOp::Not(_)    => quote! { ! #inner },
        UnOp::Neg(_)    => quote! { - #inner },
        UnOp::Deref(_)  => quote! { * #inner },
        _               => emit_todo("unsupported unary operator"),
    }
}

pub fn transpile_cast(input: &ExprCast) -> TokenStream {
    let expr = super::dispatch::go_to_rust(&input.expr);
    let ty = &input.ty;
    quote! { #expr as #ty }
}

pub fn transpile_assign(input: &ExprAssign) -> TokenStream {
    // Detect map index assignment from verbatim token stream: `count[word] = value`.
    // The Go parser captures map assignments as verbatim text like `count[word].expr`.
    let token_vec: Vec<_> = quote!(#input.left).into_iter().collect();
    let has_bracket = token_vec.iter().any(|t| {
        if let proc_macro2::TokenTree::Group(g) = t {
            g.delimiter() == proc_macro2::Delimiter::Bracket
        } else {
            false
        }
    });
    if has_bracket {
        // Extract map variable name (first token) and key (bracket content).
        if let Some(proc_macro2::TokenTree::Ident(map_name)) = token_vec.first() {
            let map_var = quote! { #map_name };
            if let Some(bracket) = token_vec.iter().find_map(|t| {
                if let proc_macro2::TokenTree::Group(g) = t {
                    Some(g.stream())
                } else { None }
            }) {
                return quote! { *::gourd::prelude::map_set_mut( #map_var, #bracket ) = #input.right };
            }
        }
    }
    let lhs = super::dispatch::go_to_rust(&input.left);
    let rhs = super::dispatch::go_to_rust(&input.right);
    quote! { #lhs = #rhs }
}

pub fn transpile_break(input: &ExprBreak) -> TokenStream {
    let label = input.label.as_ref().map(|l| quote! { #l });
    let expr = input.expr.as_ref().map(|e| super::dispatch::go_to_rust(e));
    match expr {
        Some(e) => quote! { break #label #e },
        None => quote! { break #label },
    }
}

pub fn transpile_continue(input: &ExprContinue) -> TokenStream {
    let label = input.label.as_ref().map(|l| quote! { #l });
    quote! { continue #label }
}
