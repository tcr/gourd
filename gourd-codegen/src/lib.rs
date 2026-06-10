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

pub mod debug;
pub mod scanner;
mod transpiler;
mod validate;

use proc_macro2::TokenStream;
use quote::quote;

// All handlers moved to HIR module (deprecated legacy kept as fallbacks)
use transpiler::hir::{go_to_rust_fn_hir, go_to_rust_struct_hir, go_to_rust_interface_hir, go_to_rust_receiver_fn_hir};
use transpiler::hir::go_to_rust_closure_hir;
use transpiler::hir::go_to_rust_select_hir;
use transpiler::hir::go_to_rust_switch_hir;
use transpiler::free_fn::{go_to_rust_fn, go_to_rust_struct};
pub use validate::{validate_go, validate_rust};
pub use debug::enabled;

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
    let ts: TokenStream = input.parse().unwrap_or_default();
    transpile_go(ts)
}


pub fn transpile_go(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    // Print heuristic module summary when GOURD_DEBUG is set.
    debug::print_heuristic_summary();
    // Collect all top-level declarations from the token stream.
    // Go blocks may contain multiple structs, interfaces, and functions.
    let trees: Vec<proc_macro2::TokenTree> = input.clone().into_iter().collect();
    let mut result = proc_macro2::TokenStream::new();

    let mut i = 0;
    while i < trees.len() {
        let token = &trees[i];
        match token {
            proc_macro2::TokenTree::Ident(first_ident) => {
                let first_name = first_ident.to_string();
                match first_name.as_str() {
                    "interface" => {
                        let sub = subtree(&trees, i, true);
                        result.extend(go_to_rust_interface_hir(sub));
                        i = skip_declaration(&trees, i);
                    }
                    "type" | "struct" => {
                        result.extend(go_to_rust_struct_hir(subtree(&trees, i, true)));
                        i = skip_declaration(&trees, i);
                    }
                    "switch" => {
                        // switch is a statement, not a declaration — treat as function body
                        result.extend(go_to_rust_switch_hir(subtree(&trees, i, false)));
                        i = skip_declaration(&trees, i);
                    }
                    "func" | "fn" => {
                        // Check if it's a receiver function, closure, or free function
                        if let Some(proc_macro2::TokenTree::Group(g)) = trees.get(i + 1) {
                            if g.delimiter() == proc_macro2::Delimiter::Parenthesis {
                                // Could be a receiver function OR a closure.
                                // Receiver: `func (recv Type) name(params) { body }`
                                // Closure: `func(params) { body }` or `func(params) ret_type { body }`
                                // Distinguish by checking if there's a second paren group after the identifier:
                                // Receiver: func (recv) name(params) → has Ident + Paren
                                // Closure:  func(params) int → has Ident but no second Paren
                                let has_receiver = if let Some(proc_macro2::TokenTree::Ident(_)) = trees.get(i + 2) {
                                    // Check if next token (i+3) is a parenthesis group (the params)
                                    matches!(trees.get(i + 3), Some(proc_macro2::TokenTree::Group(g)) if g.delimiter() == proc_macro2::Delimiter::Parenthesis)
                                } else {
                                    false
                                };
                                if has_receiver {
                                    result.extend(go_to_rust_receiver_fn_hir(subtree(&trees, i, true)));
                                } else {
                                    result.extend(go_to_rust_closure_hir(subtree(&trees, i, true)));
                                }
                                i = skip_declaration(&trees, i);
                            } else {
                                result.extend(go_to_rust_fn(subtree(&trees, i, true)));
                                i = skip_declaration(&trees, i);
                            }
                        } else {
                            // Edge case: `func` with no paren group — use legacy handler
                            let ts = go_to_rust_fn(subtree(&trees, i, true));
                            result.extend(ts);
                            i = skip_declaration(&trees, i);
                        }
                    }
                    "chan" => {
                        // Inline channel declaration: `chan T` → `GoChannel::<T>::new()`
                        let trees_vec: Vec<proc_macro2::TokenTree> = subtree(&trees, i, false).into_iter().collect();
                        for tree in trees_vec.iter().skip(1) {
                            match tree {
                                proc_macro2::TokenTree::Ident(ty_ident) => {
                                    let type_name = ty_ident.to_string();
                                    let mapped_type = match type_name.as_str() {
                                        "int" => quote! { i32 },
                                        "int8" => quote! { i8 },
                                        "int16" => quote! { i16 },
                                        "int32" => quote! { i32 },
                                        "int64" => quote! { i64 },
                                        "uint" => quote! { u32 },
                                        "uint8" => quote! { u8 },
                                        "uint16" => quote! { u16 },
                                        "uint32" => quote! { u32 },
                                        "uint64" => quote! { u64 },
                                        "uintptr" => quote! { usize },
                                        "byte" => quote! { u8 },
                                        "rune" => quote! { char },
                                        "float32" => quote! { f32 },
                                        "float64" => quote! { f64 },
                                        "bool" => quote! { bool },
                                        "string" => quote! { String },
                                        "error" => quote! { Box<dyn std::error::Error> },
                                        other => quote! { #other },
                                    };
                                    result.extend(quote! { GoChannel::<#mapped_type>::new() });
                                    break;
                                }
                                proc_macro2::TokenTree::Group(g) if g.delimiter() == proc_macro2::Delimiter::Bracket => {
                                    let inner: TokenStream = g.stream();
                                    let mapped_inner = match inner.to_string().as_str() {
                                        "int" => quote! { Vec<i32> },
                                        "string" => quote! { Vec<String> },
                                        "bool" => quote! { Vec<bool> },
                                        _ => quote! { Vec<#inner> },
                                    };
                                    result.extend(quote! { GoChannel::<#mapped_inner>::new() });
                                    break;
                                }
                                _ => continue,
                            }
                        }
                        i = skip_declaration(&trees, i);
                    }
                    "select" => {
                        result.extend(go_to_rust_select_hir(subtree(&trees, i, false)));
                        i = skip_declaration(&trees, i);
                    }
                    "import" => {
                        // Check if it's a grouped import: import (...)
                        let rest = &trees[i + 1..];
                        let is_grouped = !rest.is_empty()
                            && matches!(&rest[0], proc_macro2::TokenTree::Group(g) if g.delimiter() == proc_macro2::Delimiter::Parenthesis);
                        if is_grouped {
                            // Parse grouped imports: import ("strings", "fmt", ...)
                            if let proc_macro2::TokenTree::Group(g) = &rest[0] {
                                for tt in g.stream() {
                                    if let proc_macro2::TokenTree::Literal(lit) = tt {
                                        let pkg_name = lit.to_string().trim_matches('"').to_string();
                                        match pkg_name.as_str() {
                                            "strings" => result.extend(quote! { use gourd::packages::strings::*; }),
                                            "os" => result.extend(quote! { use gourd::packages::os::*; }),
                                            "time" => result.extend(quote! { use gourd::packages::time::*; }),
                                            "io" => result.extend(quote! { use gourd::packages::io::*; }),
                                            "bytes" => result.extend(quote! { use gourd::packages::bytes::*; }),
                                            "json" => result.extend(quote! { use gourd::packages::json::*; }),
                                            "math" => result.extend(quote! { use gourd::packages::math::*; }),
                                            "byte" => result.extend(quote! { use gourd::packages::byte::*; }),
                                            "fmt" => result.extend(quote! { use gourd::prelude::*; }),
                                            _ => {}
                                        }
                                    }
                                }
                                i = skip_declaration(&trees, i);
                            }
                        } else {
                            // Non-grouped import: `import "fmt"` → inline use statement
                            if trees.len() > i + 1 {
                                match &trees[i + 1] {
                                    proc_macro2::TokenTree::Ident(pkg) => {
                                        handle_import_package(&pkg.to_string(), &mut result);
                                    }
                                    proc_macro2::TokenTree::Literal(lit) => {
                                        handle_import_package(&lit.to_string().trim_matches('"'), &mut result);
                                    }
                                    _ => {}
                                }
                            }
                            i = skip_declaration(&trees, i);
                        }
                    }
                    _ => {
                        // Unknown top-level token — skip it
                        i += 1;
                    }
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    result
}

/// Extract a subtree from the token tree array starting at index `start`.
/// Returns a new TokenStream containing all tokens from `start` until
/// the end of that declaration.
fn subtree(trees: &[proc_macro2::TokenTree], start: usize, include_body: bool) -> TokenStream {
    let mut result = proc_macro2::TokenStream::new();
    let mut depth: i32 = 0;
    let mut collected = false;

    for tree in &trees[start..] {
        match tree {
            proc_macro2::TokenTree::Ident(ident) => {
                // At depth 0, if we've already collected something and see another
                // declaration keyword (func, struct, interface, chan, select),
                // stop — this is a new top-level declaration.
                if depth == 0 && collected {
                    let name = ident.to_string();
                    if matches!(name.as_str(), "func" | "fn" | "interface" | "chan" | "select" | "type") {
                        return result;
                    }
                }
                if depth == 0 && !collected {
                    collected = true;
                }
                if collected {
                    result.extend([tree.clone()]);
                }
            }
            proc_macro2::TokenTree::Literal(_) => {
                if depth == 0 && !collected {
                    collected = true;
                }
                if collected {
                    result.extend([tree.clone()]);
                }
            }
            proc_macro2::TokenTree::Group(g) => {
                // Handle brace groups at depth 0
                if depth == 0 && g.delimiter() == proc_macro2::Delimiter::Brace {
                    collected = true;
                    result.extend([proc_macro2::TokenTree::Group(g.clone())]);
                    if !include_body {
                        // For struct: return immediately
                        return result;
                    }
                    // For func: DON'T increment depth. Keep depth at 0 so the
                    // next tree's check (`depth == 0 && collected`) fires
                    // correctly when the next function's `func` keyword appears.
                    // The body is included as an atomic group — no need to scan
                    // its internals.
                    continue;
                }
                // For paren groups at depth 0 (func: receiver), keep as Group
                // so syn::parenthesized! can extract its content for ReceiverFn::parse
                if depth == 0 && g.delimiter() == proc_macro2::Delimiter::Parenthesis {
                    collected = true;
                    result.extend([proc_macro2::TokenTree::Group(g.clone())]);
                    depth += 1;
                    continue;
                }
                // For groups at depth > 0 for func:
                // Keep param groups as Group so ReceiverFn::parse can use syn::parenthesized!
                // Body: also keep as Group
                if include_body {
                    result.extend([proc_macro2::TokenTree::Group(g.clone())]);
                } else {
                    result.extend(g.stream().into_iter());
                }
                depth -= 1;
                if depth == 0 && !include_body {
                    return result;
                }
            }
            proc_macro2::TokenTree::Punct(p) => {
                if depth == 0 {
                    match p.as_char() {
                        '(' | '{' | '[' => depth += 1,
                        ')' | '}' | ']' => {
                            depth = depth.saturating_sub(1);
                            // Don't return at `)` or `]` at depth 0 — we must
                            // continue past the return type and body. Only
                            // return when we hit a new declaration keyword.
                        }
                        '<' | '>' => {
                            // Comparison operators — skip them.
                        }
                        _ => {}
                    }
                }
                if collected || depth > 0 {
                    result.extend([proc_macro2::TokenTree::Punct(p.clone())]);
                }
            }
        }
    }

    if !collected {
        return proc_macro2::TokenStream::new();
    }
    result
}

/// Handle a single import package name, emitting the appropriate `use` statement.
fn handle_import_package(pkg_name: &str, result: &mut TokenStream) {
    match pkg_name.trim_matches('"') {
        "strings" => result.extend(quote! { use gourd::packages::strings::*; }),
        "os" => result.extend(quote! { use gourd::packages::os::*; }),
        "time" => result.extend(quote! { use gourd::packages::time::*; }),
        "io" => result.extend(quote! { use gourd::packages::io::*; }),
        "bytes" => result.extend(quote! { use gourd::packages::bytes::*; }),
        "json" => result.extend(quote! { use gourd::packages::json::*; }),
        "math" => result.extend(quote! { use gourd::packages::math::*; }),
        "byte" => result.extend(quote! { use gourd::packages::byte::*; }),
        "fmt" => result.extend(quote! { use gourd::prelude::*; }),
        _ => {}
    }
}

/// Skip past a declaration starting at index `start`.
/// Returns the index of the first token after the declaration.
///
/// This function scans tokens after the declaration keyword to find where
/// the current declaration ends and the next one begins. It uses depth tracking
/// through paren groups `()` and bracket groups `[]`. For functions with bodies,
/// it skips past the body brace group. The key rule: only return when depth
/// reaches 0 after a brace group at depth 0 — closing `)` or `]` at depth 0
/// does NOT end the declaration (we must continue past the body).
fn skip_declaration(trees: &[proc_macro2::TokenTree], start: usize) -> usize {
    let mut depth: i32 = 0;
    for (i, tree) in trees[start..].iter().enumerate() {
        match tree {
            proc_macro2::TokenTree::Ident(ident) => {
                // At depth 0, if we're at an `import` keyword, skip past
                // both the import keyword and the package name identifier.
                if depth == 0 && ident.to_string() == "import" {
                    // Skip past import + one identifier (the package name)
                    // Then find the next declaration boundary
                    for (j, tree) in trees[start + i + 1..].iter().enumerate() {
                        match tree {
                            proc_macro2::TokenTree::Ident(_) => {
                                return start + i + 1 + j + 1; // skip past package name
                            }
                            proc_macro2::TokenTree::Group(g)
                                if g.delimiter() == proc_macro2::Delimiter::Brace => {
                                return start + i + 1 + j;
                            }
                            proc_macro2::TokenTree::Punct(p)
                                if p.as_char() == '(' || p.as_char() == '{' => {
                                // Skip past the delimiter — don't advance further
                                return start + i + 1 + j;
                            }
                            proc_macro2::TokenTree::Group(g)
                                if g.delimiter() == proc_macro2::Delimiter::Parenthesis => {
                                // Skip past the paren group (e.g., import list)
                                return start + i + 1 + j + 1;
                            }
                            _ => {}
                        }
                    }
                    return start + i + 2;
                }
            }
            proc_macro2::TokenTree::Group(g) => match g.delimiter() {
                proc_macro2::Delimiter::Brace => {
                    // At depth 0, a brace group means the function body.
                    // Skip past it — we've found the end of the declaration.
                    if depth == 0 {
                        return start + i + 1;
                    }
                    depth += 1;
                }
                proc_macro2::Delimiter::Parenthesis => {
                    if depth == 0 {
                        // Param/receiver group at depth 0 — part of the current
                        // declaration. Keep scanning.
                    } else {
                        depth += 1;
                    }
                }
                proc_macro2::Delimiter::Bracket => {
                    // Type annotations (e.g., `[]string`) at depth 0 — not
                    // new declarations. Keep scanning.
                    if depth == 0 {
                        // part of current declaration
                    } else {
                        depth += 1;
                    }
                }
                _ => {}
            },
            proc_macro2::TokenTree::Punct(p) => {
                if depth == 0 {
                    match p.as_char() {
                        '(' | '[' => depth += 1,
                        ')' | ']' => {
                            // Close paren/bracket at depth 0 — don't return.
                            // We need to continue past the body (brace group).
                            depth = depth.saturating_sub(1);
                        }
                        '<' | '>' => {
                            // Comparison operators — skip.
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    start + 1
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
    // If this is a string literal (starts with "), return it as-is
    if s.starts_with('"') {
        return s.to_string();
    }
    let len = s.len();
    let suffix_start = s.rfind(|c: char| !c.is_ascii_digit() && c != '.' && c != '_' && c != '-')
        .unwrap_or(len);
    s[..suffix_start].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::TokenStream;
    use quote::quote;

    // Note: Debug test harness removed — GoStruct/go_to_rust_struct legacy types not available
    // after HIR migration. Use gourd! {} blocks in actual tests to verify transpilation.

    #[test]
    fn test_struct_plus_receiver() {
        let input: TokenStream = quote! {
            struct Bar {
                value int
            }
            func (b *Bar) add(z int) int {
                b.value = b.value + z
                return b.value
            }
        };
        let result = transpile_go(input.clone());
        println!("Struct+receiver result: {}", result);
    }

    #[test]
    fn test_func_hello() {
        let result = transpile_go_text("func hello() int { return 42 }");
        println!("func hello result: '{}'", result);
    }
}

#[cfg(test)]
mod strip_tests {
    use super::*;

    #[test]
    fn test_strip_empty_string() {
        let s = "\"\""; // two quote characters
        let result = strip_literal_suffix(s);
        println!("strip empty: len={:?} result={:?} result_len={}", s.len(), result, result.len());
    }

    #[test]
    fn test_strip_hello_string() {
        let s = "\"hello\""; // seven characters
        let result = strip_literal_suffix(s);
        println!("strip hello: len={:?} result={:?} result_len={}", s.len(), result, result.len());
    }

    #[test]
    fn test_normalize_empty_string() {
        let ts: proc_macro2::TokenStream = quote::quote! { "" };
        let normalized = normalize_tokens(&ts);
        println!("normalize empty string: {:?}", normalized);
    }

    #[test]
    fn test_normalize_hello_string() {
        let ts: proc_macro2::TokenStream = quote::quote! { "hello" };
        let normalized = normalize_tokens(&ts);
        println!("normalize hello string: {:?}", normalized);
    }
}
