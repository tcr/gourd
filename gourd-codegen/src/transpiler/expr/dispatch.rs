//! Expression dispatch: top-level `go_to_rust()` and `go_to_rust_pattern()`.
//!
//! Routes `syn::Expr` variants to the appropriate `transpile_*` handler.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Expr;

/// Emit a compile-time error for forms we don't support.
/// Includes the Go source text when available for better diagnostics.
pub(crate) fn emit_todo(msg: &str) -> TokenStream {
    quote! { {
        compile_error!(concat!("TODO: Go transpile — ", #msg, " — check Go code context"));
        unreachable!()
    }}
}

/// Dispatch the AST per expression node.
pub fn go_to_rust(input: &Expr) -> TokenStream {
    match input {
        Expr::Lit(e)            => super::literals::transpile_lit(e),
        Expr::Binary(e)         => super::operators::transpile_binary(e),
        Expr::Unary(e)          => super::operators::transpile_unary(e),
        Expr::Path(e)           => super::literals::transpile_path(e),
        Expr::Call(e)           => super::calls::transpile_call(e),
        Expr::Paren(e)          => super::literals::transpile_paren(e),
        Expr::Group(e)          => go_to_rust(&e.expr),
        Expr::Block(e)          => super::control_flow::transpile_block(e),
        Expr::If(e)             => super::control_flow::transpile_if(e),
        Expr::Range(e)          => super::control_flow::transpile_range(e),
        Expr::Index(e)          => super::calls::transpile_index(e),
        Expr::Array(e)          => super::literals::transpile_array(e),
        Expr::Loop(e)           => super::control_flow::transpile_loop(e),
        Expr::ForLoop(e)        => super::control_flow::transpile_for_loop(e),
        Expr::While(e)          => super::control_flow::transpile_while(e),
        Expr::MethodCall(c)     => super::calls::transpile_method_call(c),
        Expr::Field(e)          => super::calls::transpile_field(e),
        Expr::Let(e)            => super::control_flow::transpile_let(e),
        Expr::Tuple(e)          => super::control_flow::transpile_tuple(e),
        Expr::Cast(e)           => super::operators::transpile_cast(e),
        Expr::Assign(e)         => super::operators::transpile_assign(e),
        Expr::Break(e)          => super::operators::transpile_break(e),
        Expr::Continue(e)       => super::operators::transpile_continue(e),
        Expr::Reference(e)      => {
            // Rust `&expr` — Go `&x` is address-of, but in the transpiled
            // output `&x` becomes a reference in Rust, which is fine.
            let inner = go_to_rust(&e.expr);
            quote! { &#inner }
        }
        Expr::Return(e)         => super::control_flow::transpile_return(e),
        Expr::Macro(e)          => super::calls::go_to_rust_macro(e),
        Expr::Verbatim(tokens)  => super::literals::transpile_verbatim(tokens),
        Expr::Struct(e)         => super::structs::transpile_struct(e),
        Expr::Match(e)          => super::control_flow::transpile_match(e),
        _                       => emit_todo("unsupported Go form"),
    }
}

/// Expression transpilation for Rust **match patterns**.
///
/// Unlike `go_to_rust`, this keeps string literals as `&str` patterns
/// (raw `"..."` literal) instead of wrapping them in `String::from(...)`,
/// because Rust match arms require patterns, not expressions.
pub fn go_to_rust_pattern(input: &Expr) -> TokenStream {
    match input {
        Expr::Lit(e)            => super::literals::transpile_lit_pattern(e),
        Expr::Path(e)           => super::literals::transpile_path(e),
        Expr::Paren(e)          => go_to_rust_pattern(&e.expr),
        Expr::Group(e)          => go_to_rust_pattern(&e.expr),
        Expr::Tuple(e)          => super::control_flow::transpile_tuple(e),
        Expr::Verbatim(tokens)  => super::literals::transpile_verbatim(tokens),
        _                       => emit_todo("unsupported match pattern"),
    }
}

#[cfg(test)]
mod test_expr_parse {
    use syn::Expr;

    #[test]
    fn test_parse_ch() {
        let input = "ch";
        match syn::parse_str::<Expr>(input) {
            Ok(expr) => println!("'ch' -> {}", quote::quote!(#expr)),
            Err(e) => println!("'ch' -> Error: {}", e),
        }
    }

    #[test]
    fn test_parse_ch_lt_value() {
        let input = "ch <- value";
        match syn::parse_str::<Expr>(input) {
            Ok(expr) => println!("'ch <- value' -> {}", quote::quote!(#expr)),
            Err(e) => println!("'ch <- value' -> Error: {}", e),
        }
    }
}

#[cfg(test)]
mod test_index_parse {
    use syn::Expr;
    use quote::quote;

    /// Test that syn parses `a[1..3]` (Rust slice range) as Expr::Index
    /// with an Expr::Range inside — this is what the Go→Rust preprocessor
    /// produces after converting `a[1:3]` → `a[1..3]`.
    #[test]
    fn test_parse_index_range() {
        let expr: Expr = syn::parse_str("a[1..3]").unwrap();
        println!("a[1..3] parsed as: {}", quote! { #expr });
        match &expr {
            Expr::Index(idx) => {
                println!("Index expr: seq={}, index={}",
                    quote! { #idx.expr },
                    quote! { #idx.index });
                println!("index type: {:?}", std::mem::discriminant(&*idx.index));
                // The index should be an Expr::Range
                assert!(matches!(*idx.index, Expr::Range(_)));
            }
            other => panic!("Expected Index, got {:?}", quote! { #other }),
        }
    }

    #[test]
    fn test_parse_index_single() {
        let expr: Expr = syn::parse_str("a[1]").unwrap();
        match &expr {
            Expr::Index(idx) => {
                println!("a[1]: seq={}, index={}",
                    quote! { #idx.expr },
                    quote! { #idx.index });
            }
            other => println!("Not Index: {:?}", quote! { #other }),
        }
    }

    /// Test that syn parses `a[..]` (Rust full slice) as Expr::Index
    /// — this is what the Go→Rust preprocessor produces after `a[:]` → `a[..]`.
    #[test]
    fn test_parse_full_slice() {
        let expr: Expr = syn::parse_str("a[..]").unwrap();
        match &expr {
            Expr::Index(idx) => {
                println!("a[..]: seq={}, index={}",
                    quote! { #idx.expr },
                    quote! { #idx.index });
                assert!(matches!(*idx.index, Expr::Range(_)));
            }
            other => panic!("Expected Index, got {:?}", quote! { #other }),
        }
    }
}
