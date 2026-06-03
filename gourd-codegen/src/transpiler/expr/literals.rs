//! Literal and path transpilation: `Lit`, `Path`, `Paren`, `Array`, `Verbatim`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{ExprArray, ExprLit, ExprParen, ExprPath};

use super::super::parsing::ElemParser;

pub fn transpile_lit(input: &ExprLit) -> TokenStream {
    let lit = &input.lit;
    match lit {
        syn::Lit::Str(s) => quote! { ::std::string::String::from(#s) },
        _                => quote! { #lit },
    }
}

/// Pattern variant: keep string literals as `&str` patterns.
pub fn transpile_lit_pattern(input: &ExprLit) -> TokenStream {
    let lit = &input.lit;
    match lit {
        syn::Lit::Str(s) => quote! { #s },  // &str pattern, not String::from
        _                => quote! { #lit },
    }
}

pub fn transpile_path(input: &ExprPath) -> TokenStream {
    let p = &input.path;
    match p.get_ident() {
        Some(ident) => match ident.to_string().as_str() {
            "nil"   => quote! { None },
            "true"  => quote! { true },
            "false" => quote! { false },
            _       => quote! { #p },
        },
        None => quote! { #p },
    }
}

pub fn transpile_paren(input: &ExprParen) -> TokenStream {
    let inner = super::dispatch::go_to_rust(&input.expr);
    quote! { ( #inner ) }
}

pub fn transpile_array(input: &ExprArray) -> TokenStream {
    let elems: Vec<_> = input.elems.iter().map(super::dispatch::go_to_rust).collect();
    if elems.is_empty() {
        // In Go slice literals like `[]int{ 1, 2, 3 }`, syn parses `[]`
        // as an empty array expression. The actual slice elements come
        // from the `Expr::Verbatim` handling below.
        quote! { vec![] }
    } else {
        quote! { [#(#elems),*] }
    }
}

/// Handle `Expr::Verbatim` tokens produced by syn when it can't fully
/// parse Go slice/map literals or anonymous functions.
pub fn transpile_verbatim(tokens: &proc_macro2::TokenStream) -> TokenStream {
    use proc_macro2::TokenTree;

    // Check for anonymous Go function: `func(params) ret { body }`
    // or `func(params) { body }` — no return type.
    if let Some(closure) = super::closures::parse_closure(tokens) {
        return super::closures::closure_to_rust(&closure);
    }

    // Check for slice/map literals
    for tt in tokens.clone() {
        if let TokenTree::Group(g) = tt
            && g.delimiter() == proc_macro2::Delimiter::Brace
        {
            let brace_content = g.stream();
            let parser: ElemParser = syn::parse2(brace_content).unwrap_or_default();
            let elems: Vec<_> = parser.elems.iter().map(|expr| super::dispatch::go_to_rust(expr)).collect();
            return quote! { vec![ #(#elems),* ] };
        }
    }

    // Check for Go slice slicing: `slice[1:]`, `slice[1:3]`, `slice[:3]`
    // This is verbatim because syn can't parse Go's `[start:end]` syntax
    if let Some(sliced) = try_parse_slice_slicing(tokens) {
        return sliced;
    }

    // No brace group — emit raw tokens (simple literals)
    quote! { #tokens }
}

/// Try to parse Go slice slicing from verbatim tokens: `slice[1:]`, `slice[1:3]`, `slice[:3]`
fn try_parse_slice_slicing(tokens: &proc_macro2::TokenStream) -> Option<TokenStream> {
    use proc_macro2::TokenTree;

    let mut token_iter = tokens.clone().into_iter().peekable();

    // Extract identifier or paren group before `[`
    let base = match token_iter.next() {
        Some(TokenTree::Ident(base_id)) => quote! { #base_id },
        Some(TokenTree::Group(g)) => {
            if g.delimiter() == proc_macro2::Delimiter::Parenthesis {
                let inner = g.stream();
                return Some(quote! { (#inner)[1..].to_vec() });
            }
            return None;
        }
        _ => return None,
    };

    // Expect `[`
    let open_bracket = match token_iter.next() {
        Some(TokenTree::Punct(p)) if p.as_char() == '[' => p,
        _ => return None,
    };
    drop(open_bracket);

    // Check for `:` at the start (e.g., `[:3]`)
    let start = match token_iter.peek() {
        Some(TokenTree::Punct(p)) if p.as_char() == ':' => {
            token_iter.next();
            quote! { 0 }
        }
        Some(_) => {
            let mut start_tokens = proc_macro2::TokenStream::new();
            while let Some(tt) = token_iter.peek() {
                if let TokenTree::Punct(p) = tt {
                    if p.as_char() == ':' {
                        token_iter.next();
                        break;
                    } else {
                        start_tokens.extend([token_iter.next().unwrap()]);
                    }
                } else {
                    start_tokens.extend([token_iter.next().unwrap()]);
                }
            }
            start_tokens
        }
        None => return None,
    };

    // Parse end index (if present, otherwise end of slice)
    let end = if token_iter.peek().is_some() {
        let mut end_tokens = proc_macro2::TokenStream::new();
        while let Some(tt) = token_iter.next() {
            if let TokenTree::Punct(p) = &tt {
                if p.as_char() == ']' {
                    break;
                }
            }
            end_tokens.extend([tt]);
        }
        end_tokens
    } else {
        quote! {}
    };

    if end.is_empty() {
        // `it[1:]` → `it[1..].to_vec()`
        Some(quote! { (#base)[(#start)..].to_vec() })
    } else {
        // `it[1:3]` → `it[1..3].to_vec()`
        Some(quote! { (#base)[(#start)..(#end)].to_vec() })
    }
}
