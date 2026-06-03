//! Helper functions for validation.
//!
//! Provides functions for converting token streams to strings and
//! wrapping code in minimal harnesses.

use proc_macro2::TokenStream;

/// Convert token stream to string, preserving original formatting.
/// Adds semicolons between statements when needed for Go validation.
pub(crate) fn ts_to_string(ts: &TokenStream) -> String {
    use proc_macro2::TokenTree;
    let mut s = String::new();
    let mut need_space = false;
    let mut expect_statement_end = false;

    for tree in ts.clone().into_iter() {
        match tree {
            TokenTree::Ident(id) => {
                let id_str = id.to_string();
                // Insert semicolon before statement keywords when transitioning
                // from a statement-ending context to a new statement
                if expect_statement_end
                    && matches!(id_str.as_str(), "if" | "for" | "switch" | "return" | "break" | "continue")
                {
                    s.push(';');
                }
                if need_space && !s.is_empty() && !s.ends_with(' ') {
                    s.push(' ');
                }
                s.push_str(&id_str);
                need_space = true;
                // These keywords end a statement context
                expect_statement_end = !matches!(id_str.as_str(),
                    "else" | "case" | "default" | "map" | "struct" | "func");
            }
            TokenTree::Literal(lit) => {
                if need_space && !s.is_empty() && !s.ends_with(' ') {
                    s.push(' ');
                }
                s.push_str(&lit.to_string());
                need_space = false;
                expect_statement_end = true;
            }
            TokenTree::Group(g) => {
                if need_space && !s.is_empty() && !s.ends_with(' ') {
                    s.push(' ');
                }
                let inner = ts_to_string(&g.stream());
                match g.delimiter() {
                    proc_macro2::Delimiter::Parenthesis => s.push_str(&format!("({})", inner)),
                    proc_macro2::Delimiter::Brace => s.push_str(&format!("{{{}}}", inner)),
                    proc_macro2::Delimiter::Bracket => s.push_str(&format!("[{}]", inner)),
                    proc_macro2::Delimiter::None => s.push_str(&inner),
                }
                need_space = true;
                // A closing brace often ends a statement
                expect_statement_end = matches!(g.delimiter(),
                    proc_macro2::Delimiter::Brace | proc_macro2::Delimiter::Bracket | proc_macro2::Delimiter::Parenthesis);
            }
            TokenTree::Punct(p) => {
                let ch = p.as_char();
                s.push(ch);
                need_space = true;
                // Semicolons are explicit statement terminators
                if ch == ';' {
                    expect_statement_end = false;
                }
            }
        }
    }
    s
}

/// Wrap Go declarations in a minimal `package main` harness.
pub(crate) fn go_to_main_harness(go_code: &TokenStream) -> String {
    let code = ts_to_string(go_code);

    // `quote!` adds spaces around punctuation (e.g., `func hello ( ) string`).
    // Fix: remove space before `(`, `)`, `{`, `}`, `[`, `]`.
    // `func hello ( ) string` → `func hello() string`
    let code = fix_quote_spaces(&code);

    // Build the harness: wrap declarations in package main with a main() function.
    // Go rejects `package main` without imports or a main() function.
    let mut harness = String::new();
    harness.push_str("package main\n\n");
    harness.push_str(&code);
    harness.push_str("\n\nfunc main() {\n}");
    harness
}

/// Wrap Rust declarations in a minimal `fn main()` harness.
pub(crate) fn rust_to_main_harness(rust_code: &TokenStream) -> String {
    let code = ts_to_string(rust_code);

    // `quote!` adds spaces around punctuation (e.g., `fn hello ( ) { }`).
    // Fix: remove space before `(`, `)`, `{`, `}`.
    // `fn hello ( ) { }` → `fn hello() { }`
    let code = fix_quote_spaces(&code);
    format!("fn main() {{}}\n\n{code}\n")
}

fn fix_quote_spaces(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut prev: Option<char> = None;

    while let Some(c) = chars.next() {
        // Remove space before closing delimiters: ` ) ` → `)`
        if (c == ')' || c == '}' || c == ']') && prev == Some(' ') {
            result.pop();
        }
        result.push(c);
        prev = Some(c);
    }
    result
}
