use proc_macro::TokenStream;
use syn::parse_macro_input;
use syn::Expr;

mod transpiler;

/// Re-export the expression macro so `use gourd_codegen::go_expr!` works.
#[proc_macro]
pub fn go_expr(input: TokenStream) -> TokenStream {
    let expr = parse_macro_input!(input as Expr);
    transpiler::go_to_rust(&expr).into()
}

/// Top-level macro for Go declarations.
/// Dispatches to the appropriate transpiler based on input pattern:
///   1. `func (recv Type) name() { ... }` → `impl Type { fn name(&self) { ... } }`
///   2. `struct Name { field type }` → `struct Name { pub field: Type }`
///   3. `func name() { ... }` → `fn name() { ... }`
#[proc_macro]
pub fn go(input: TokenStream) -> TokenStream {
    let tokens: proc_macro2::TokenStream = input.into();
    let mut iter = tokens.clone().into_iter();

    // Peek first token to decide dispatch path
    match iter.next() {
        Some(proc_macro2::TokenTree::Ident(first_ident)) => {
            let first_name = first_ident.to_string();
            match first_name.as_str() {
                "struct" => {
                    transpiler::go_to_rust_struct(tokens).into()
                }
                "func" | "fn" => {
                    // Check if second token is `(Parenthesis Group)` → receiver function
                    if let Some(proc_macro2::TokenTree::Group(g)) = iter.next() {
                        if g.delimiter() == proc_macro2::Delimiter::Parenthesis {
                            transpiler::go_to_rust_receiver_fn(tokens).into()
                        } else {
                            transpiler::go_to_rust_fn(tokens).into()
                        }
                    } else {
                        transpiler::go_to_rust_fn(tokens).into()
                    }
                }
                _ => transpiler::go_to_rust_fn(tokens).into(),
            }
        }
        _ => transpiler::go_to_rust_fn(tokens).into(),
    }
}
