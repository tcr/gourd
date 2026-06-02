//! Core transpilation library for Go → Rust conversion.
//!
//! This crate contains the transpilation logic used by the `gourd` proc-macro.
//! It exposes `transpile_go()` for direct inspection and testing.
//!
//! ## Public API
//!
//! - `transpile_go(input)` — transpile a Go declaration to Rust tokens
//! - `transpile_go_fn(input)` — transpile a free Go function
//! - `transpile_go_struct(input)` — transpile a Go struct
//! - `transpile_go_switch(input)` — transpile a Go switch statement
//! - `transpile_go_receiver_fn(input)` — transpile a receiver function
//! - `verify_short(attr, input)` — verify Go→Rust transpilation output
//! - `normalize_tokens(tokens)` — normalize token streams for comparison

mod transpiler;
mod validate;

pub use transpiler::free_fn::{go_to_rust_fn, go_to_rust_interface, go_to_rust_struct, go_to_rust_switch};
pub use transpiler::funcs::go_to_rust_receiver_fn;
pub use validate::{validate_go, validate_rust};

/// Public transpilation entry point.
///
/// Dispatches Go declarations to the appropriate transpiler based on
/// the first token:
///   - `func (recv Type) name() { ... }` → receiver function impl
///   - `struct Name { field type }` → Rust struct
///   - `switch x { ... }` → Rust match/if-else
///   - `func name() { ... }` → free function
/// Transpile raw Go code text to Rust token stream.
///
/// This is the CLI-facing entry point. Takes raw Go code as a string
/// (optionally wrapped in the macro invocation form) and dispatches to the
/// appropriate transpiler based on the first token.
///
/// # Examples
///
/// 
pub fn transpile_go_text(input: &str) -> proc_macro2::TokenStream {
    use proc_macro2::TokenStream;
    use quote::ToTokens;
    let ts: TokenStream = input.parse().unwrap_or_default();
    transpile_go(ts)
}


pub fn transpile_go(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let mut iter = input.clone().into_iter();

    // Peek first token to decide dispatch path
    match iter.next() {
        Some(token) => match token {
            proc_macro2::TokenTree::Ident(first_ident) => {
                let first_name = first_ident.to_string();
                match first_name.as_str() {
                    "struct" => {
                        go_to_rust_struct(input)
                    }
                    "switch" => {
                        go_to_rust_switch(input)
                    }
                    "func" | "fn" => {
                        let mut iter2 = input.clone().into_iter().skip(1);
                        if let Some(proc_macro2::TokenTree::Group(g)) = iter2.next() {
                            if g.delimiter() == proc_macro2::Delimiter::Parenthesis {
                                go_to_rust_receiver_fn(input)
                            } else {
                                go_to_rust_fn(input)
                            }
                        } else {
                            go_to_rust_fn(input)
                        }
                    }
                    _ => go_to_rust_fn(input),
                }
            }
            _ => go_to_rust_fn(input),
        },
        None => proc_macro2::TokenStream::new(),
    }
}

/// Short-form verify: `#[go_verify({ expected_rust_tokens })]`
///
/// The attribute receives a brace group `{ ... }` containing expected
/// Rust tokens. Extracts the Go code from the `go!` item,
/// transpiles it, and compares against the expected tokens.
///
/// On match: emits the transpiled Rust tokens (go! is consumed by the attribute).
/// On mismatch: emits compile_error!("expected vs actual mismatch.")
///
/// Usage:
/// ```ignore
/// #[go_verify({
///     fn go_abs(n: i32) -> i32 {
///         let mut ret = n;
///         if n < 0 { ret = -n; }
///         ret
///     }
/// })]
/// go! {
///     func goAbs(n int) int {
///         ret := n
///         if n < 0 { ret = -n }
///         return ret
///     }
/// }
/// ```
pub fn verify_short(attr: proc_macro2::TokenStream, input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    // Extract expected tokens from the attribute input.
    // The attr should be `{ expected_rust_tokens }` (brace group).
    let expected_tokens = if attr.is_empty() {
        proc_macro2::TokenStream::new()
    } else {
        let attr_trees: Vec<proc_macro2::TokenTree> = attr.clone().into_iter().collect();
        // Check if the first token is a brace group (the short form)
        if let Some(proc_macro2::TokenTree::Group(g)) = attr_trees.first() {
            if g.delimiter() == proc_macro2::Delimiter::Brace {
                g.stream()
            } else {
                // Fallback: search for `verify = { }` inside
                parse_verify_from_attr(&attr)
            }
        } else {
            proc_macro2::TokenStream::new()
        }
    };

    // Extract the Go code from the `go!` item.
    // The input is: `go!` → three token trees: Ident("go"), Punct("!"), Group(Brace, ...)
    let go_input = extract_go_block_from_input(&input);

    if go_input.is_empty() {
        // Can't find a go block — just pass through
        return input;
    }

    if expected_tokens.is_empty() {
        // No verify block — just transpile normally
        return transpile_go(go_input);
    }

    // Validate expected tokens as valid Rust by trying to parse them as syn::File.
    // If they don't parse, emit a compile_error so the user knows the verify block
    // contains invalid Rust syntax (not just a mismatch).
    if syn::parse2::<syn::File>(expected_tokens.clone()).is_err() {
        let expected_str = normalize_tokens(&expected_tokens).join(" ");
        return quote::quote! {
            compile_error!(concat!(
                "`verify_rust_output` expected block is not valid Rust:\n",
                "  ", #expected_str
            ))
        };
    }

    // Transpile the Go block and compare
    let transpiled = transpile_go(go_input);
    let expected_normalized = normalize_tokens(&expected_tokens);
    let actual_normalized = normalize_tokens(&transpiled);

    if expected_normalized == actual_normalized {
        // Match! Pass through original input so `go!` handles emission.
        input
    } else {
        // Mismatch! Emit a compile_error.
        let expected_str = expected_normalized.join(" ");
        let actual_str = actual_normalized.join(" ");
        quote::quote! {
            compile_error!(concat!(
                "Go→Rust `verify_rust_output` mismatch:\n",
                "  expected: ", #expected_str, "\n",
                "  actual:   ", #actual_str
            ))
        }
    }
}

/// Extract the Go code block from the attribute input.
/// Handles the `go` macro, raw brace groups, and `go!(x) { ... }` variants.
fn extract_go_block_from_input(input: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    use proc_macro2::TokenTree;

    let trees: Vec<TokenTree> = input.clone().into_iter().collect();

    // Case 1: `go!(...)` with brace-delimited body
    if trees.len() >= 3 {
        if let (TokenTree::Ident(id), TokenTree::Punct(p), TokenTree::Group(g)) =
            (&trees[0], &trees[1], &trees[2])
        {
            if id == "go" && p.as_char() == '!' && g.delimiter() == proc_macro2::Delimiter::Brace {
                return g.stream();
            }
        }
    }

    // Case 2: `go! (x) { ... }` — Ident("go"), Punct("!"), Group(Paren), Group(Brace, ...)
    if trees.len() >= 4 {
        if let (TokenTree::Ident(id), TokenTree::Punct(p), TokenTree::Group(_), TokenTree::Group(g)) =
            (&trees[0], &trees[1], &trees[2], &trees[3])
        {
            if id == "go" && p.as_char() == '!' && g.delimiter() == proc_macro2::Delimiter::Brace {
                return g.stream();
            }
        }
    }

    // Case 3: bare `{ ... }` — single brace group
    if trees.len() == 1 {
        if let TokenTree::Group(g) = &trees[0] {
            if g.delimiter() == proc_macro2::Delimiter::Brace {
                return g.stream();
            }
        }
    }

    proc_macro2::TokenStream::new()
}

/// Parse `verify = { }` from a longer-form attribute input.
fn parse_verify_from_attr(attr_stream: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    use proc_macro2::TokenTree;

    let trees: Vec<TokenTree> = attr_stream.clone().into_iter().collect();
    let mut i = 0;

    while i < trees.len() {
        let is_verify_ident = match &trees[i] {
            TokenTree::Ident(id) => id == "verify",
            TokenTree::Group(g) if g.delimiter() == proc_macro2::Delimiter::None => {
                g.stream().into_iter().any(|t| {
                    matches!(&t, TokenTree::Ident(id) if id == "verify")
                })
            }
            _ => false,
        };

        if is_verify_ident {
            let brace_offset = if i + 1 < trees.len() {
                match &trees[i + 1] {
                    TokenTree::Punct(p) if p.as_char() == '=' => 2,
                    _ => 1,
                }
            } else {
                break;
            };

            let brace_idx = i + brace_offset;
            if brace_idx < trees.len() {
                if let TokenTree::Group(brace_group) = &trees[brace_idx] {
                    if brace_group.delimiter() == proc_macro2::Delimiter::Brace {
                        return brace_group.stream();
                    }
                }
            }
            break;
        }
        i += 1;
    }

    // Fallback: if the entire attr is a brace group
    if trees.len() == 1 {
        if let TokenTree::Group(g) = &trees[0] {
            if g.delimiter() == proc_macro2::Delimiter::Brace {
                return g.stream();
            }
        }
    }

    proc_macro2::TokenStream::new()
}

/// Normalize a token stream for comparison: flatten into a vector of strings.
/// Recursively handles groups, strips literal suffixes, and keeps punctuation.
pub fn normalize_tokens(tokens: &proc_macro2::TokenStream) -> Vec<String> {
    use proc_macro2::TokenTree;

    let mut result = Vec::new();
    for tree in tokens.clone().into_iter() {
        match tree {
            TokenTree::Ident(id) => result.push(id.to_string()),
            TokenTree::Literal(lit) => result.push(strip_literal_suffix(&lit.to_string())),
            TokenTree::Punct(p) => result.push(p.as_char().to_string()),
            TokenTree::Group(g) => {
                let inner = normalize_tokens(&g.stream());
                let (open, close) = match g.delimiter() {
                    proc_macro2::Delimiter::Parenthesis => (String::from("("), String::from(")")),
                    proc_macro2::Delimiter::Brace => (String::from("{"), String::from("}")),
                    proc_macro2::Delimiter::Bracket => (String::from("["), String::from("]")),
                    proc_macro2::Delimiter::None => return inner, // Transparent
                };
                result.push(open);
                result.extend(inner);
                result.push(close);
            }
        }
    }
    result
}

fn strip_literal_suffix(s: &str) -> String {
    let len = s.len();
    let suffix_start = s.rfind(|c: char| !c.is_ascii_digit() && c != '.' && c != '_' && c != '-')
        .map(|i| i + 1)
        .unwrap_or(len);
    s[..suffix_start].to_string()
}
