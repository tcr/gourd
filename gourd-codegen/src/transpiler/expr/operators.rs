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
