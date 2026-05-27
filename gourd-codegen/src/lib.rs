use proc_macro::TokenStream;
use proc_macro2::{TokenStream as TokenStream2, TokenTree, Delimiter};
use syn::parse_macro_input;
use syn::Expr;

mod transpiler;

/// Re-export the expression macro so `use gourd_codegen::go_expr!` works.
/// Tries to parse as Go slice/map literal first, then falls back to
/// Go expression parsing.
#[proc_macro]
pub fn go_expr(input: TokenStream) -> TokenStream {
    let tokens: TokenStream2 = input.clone().into();

    // Check if the tokens form a Go slice literal: []Type{...} or []{...}
    // A slice literal starts with `[` (as a bracket group).
    if check_is_slice_literal(&tokens) {
        if let Ok(slice_lit) = transpiler::slices::parse_go_slice(&tokens) {
            return transpiler::slices::go_to_rust_slice(&slice_lit).into();
        }
    }

    // Check if the tokens form a Go map literal: map[K]V{...}
    if check_is_map_literal(&tokens) {
        if let Ok(map_lit) = transpiler::slices::parse_go_map(&tokens) {
            return transpiler::slices::go_to_rust_map(&map_lit).into();
        }
    }

    // Fall back to standard Go expression parsing
    let tokens2: proc_macro::TokenStream = input;
    let expr = parse_macro_input!(tokens2 as Expr);
    transpiler::go_to_rust(&expr).into()
}

/// Check if tokens look like a Go slice literal `[Type]{elems}` or `[]{elems}`
fn check_is_slice_literal(tokens: &TokenStream2) -> bool {
    let mut iter = tokens.clone().into_iter();
    match iter.next() {
        Some(TokenTree::Group(g)) => g.delimiter() == Delimiter::Bracket,
        _ => false,
    }
}

/// Check if tokens look like a Go map literal `map[K]V{entries}`
fn check_is_map_literal(tokens: &TokenStream2) -> bool {
    let mut iter = tokens.clone().into_iter();
    match iter.next() {
        Some(TokenTree::Ident(ident)) => ident.to_string() == "map",
        _ => false,
    }
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
                            transpiler::funcs::go_to_rust_receiver_fn(tokens).into()
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
