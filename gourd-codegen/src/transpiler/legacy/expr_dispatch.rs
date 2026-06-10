//! Expression dispatch: top-level `go_to_rust()` and `go_to_rust_pattern()`.
//!
//! Routes `syn::Expr` variants to the appropriate `transpile_*` handler.

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::Expr;

/// Emit a compile-time error for forms we don't support.
pub(crate) fn emit_todo(msg: &str) -> TokenStream {
    emit_todo_with_span(msg, Span::call_site())
}

/// Emit a compile-time error for forms we don't support, with span information.
pub(crate) fn emit_todo_with_span(msg: &str, span: Span) -> TokenStream {
    let msg_span = quote_spanned!(span => TODO: Go transpile - #msg - check Go code context);
    quote! {
        {
            compile_error!(#msg_span);
            unreachable!()
        }
    }
}

/// Dispatch the AST per expression node.
pub fn go_to_rust(input: &Expr) -> TokenStream {
    match input {
        Expr::Lit(e)            => super::expr_literals::transpile_lit(e),
        Expr::Binary(e)         => super::expr_operators::transpile_binary(e),
        Expr::Unary(e)          => super::expr_operators::transpile_unary(e),
        Expr::Path(e)           => super::expr_literals::transpile_path(e),
        Expr::Call(e)           => super::expr_calls::transpile_call(e),
        Expr::Paren(e)          => super::expr_literals::transpile_paren(e),
        Expr::Group(e)          => go_to_rust(&e.expr),
        Expr::Block(e)          => super::expr_control_flow::transpile_block(e),
        Expr::If(e)             => super::expr_control_flow::transpile_if(e),
        Expr::Range(e)          => super::expr_control_flow::transpile_range(e),
        Expr::Index(e)          => super::expr_calls::transpile_index(e),
        Expr::Array(e)          => super::expr_literals::transpile_array(e),
        Expr::Loop(e)           => super::expr_control_flow::transpile_loop(e),
        Expr::ForLoop(e)        => super::expr_control_flow::transpile_for_loop(e),
        Expr::While(e)          => super::expr_control_flow::transpile_while(e),
        Expr::MethodCall(c)     => super::expr_calls::transpile_method_call(c),
        Expr::Field(e)          => super::expr_calls::transpile_field(e),
        Expr::Let(e)            => super::expr_control_flow::transpile_let(e),
        Expr::Tuple(e)          => super::expr_control_flow::transpile_tuple(e),
        Expr::Cast(e)           => super::expr_operators::transpile_cast(e),
        Expr::Assign(e)         => super::expr_operators::transpile_assign(e),
        Expr::Break(e)          => super::expr_operators::transpile_break(e),
        Expr::Continue(e)       => super::expr_operators::transpile_continue(e),
        Expr::Return(e)         => super::expr_control_flow::transpile_return(e),
        Expr::Macro(e)          => super::expr_calls::go_to_rust_macro(e),
        Expr::Verbatim(e)       => super::expr_literals::transpile_verbatim(e),
        Expr::Reference(e)      => {
            let inner = go_to_rust(&e.expr);
            quote! { &#inner }
        },
        Expr::Match(m)          => super::expr_control_flow::transpile_match(m),
        Expr::Struct(e)         => super::expr_structs::transpile_struct(e),
        // Catch-all for any unhandled expression variants
        e => emit_todo(&format!("unsupported expression type: {}", quote! { #e })),
    }
}

/// Same as `go_to_rust` but keeps string literals as `&str` patterns
/// for use in match arms where strings are match patterns, not expressions.
pub fn go_to_rust_pattern(input: &Expr) -> TokenStream {
    match input {
        Expr::Lit(e)            => super::expr_literals::transpile_lit_pattern(e),
        Expr::Path(e)           => super::expr_literals::transpile_path(e),
        Expr::Paren(e)          => go_to_rust_pattern(&e.expr),
        Expr::Group(e)          => go_to_rust_pattern(&e.expr),
        Expr::Tuple(e)          => super::expr_control_flow::transpile_tuple(e),
        Expr::Verbatim(tokens)  => super::expr_literals::transpile_verbatim(tokens),
        _ => go_to_rust(input), // Fallback to regular handling
    }
}
