//! Switch statement transpilation.
//!
//! Converts Go switch statements to Rust `match` expressions or
//! if-else chains (when no selector is present).

use crate::transpiler::legacy::stmt_to_rust::go_stmt_to_rust;
use crate::transpiler::legacy::expr_dispatch::{go_to_rust, go_to_rust_pattern};
use crate::transpiler::hir::ast::Switch;
use proc_macro2::TokenStream;
use quote::quote;

/// Top-level: parse and transpile a Go switch statement to Rust.
pub fn go_to_rust_switch(input: TokenStream) -> TokenStream {
    match syn::parse2::<Switch>(input) {
        Ok(switch) => transpile_switch(&switch),
        Err(e) => e.to_compile_error(),
    }
}

pub fn transpile_switch(switch: &Switch) -> TokenStream {
    crate::debug_println!("DEBUG: transpile_switch called, selector={:?}, cases={}, default_stmts={}", 
        switch.selector.is_some(), switch.cases.len(), switch.default_stmts.len());
    // Build match arms from case expressions
    let mut arms = Vec::new();
    crate::debug_println!("DEBUG: transpile_switch - building {} arms", switch.cases.len());

    for case in &switch.cases {
        crate::debug_println!("DEBUG: transpile_switch - processing case, exprs={}, stmts={}", 
            case.exprs.len(), case.stmts.len());
        if case.exprs.is_empty() {
            // Empty exprs means this is a default-like case
            // but we handle default separately
            crate::debug_println!("DEBUG: transpile_switch - case has empty exprs, skipping");
            continue;
        }

        // Case expressions become match patterns (string literals stay as &str)
        let pattern: Vec<_> = case.exprs.iter().map(|e| go_to_rust_pattern(e)).collect();
        let body: Vec<_> = case.stmts.iter().map(|s| go_stmt_to_rust(s)).collect();

        // Single or multi-expression case
        // Multi-expr: `case 1, 2, 3:` → `1 | 2 | 3 =>`
        let arm_tokens = quote! { #(#pattern)|* => { #(#body);* } };
        crate::debug_println!("DEBUG: transpile_switch - adding arm with pattern: {}", arm_tokens);
        arms.push(arm_tokens);
    }
    crate::debug_println!("DEBUG: transpile_switch - built {} arms", arms.len());

    // Handle default case with `_` pattern
    if !switch.default_stmts.is_empty() {
        let default_body: Vec<_> = switch.default_stmts.iter()
            .map(|s| go_stmt_to_rust(s))
            .collect();
        arms.push(quote! { _ => { #(#default_body);* } });
    }

    // When there's no selector, use if-else chain (common for bool switches)
    if switch.selector.is_none() {
        // Build if-else chain: `if cond { body } else if cond { body } else { default }`
        if switch.cases.is_empty() && switch.default_stmts.is_empty() {
            return quote! { () };
        }

        // Handle the first case as the initial `if` (no `else` prefix)
        if !switch.cases.is_empty() {
            let first_case = &switch.cases[0];
            let first_conds: Vec<_> = first_case.exprs.iter().map(|e| go_to_rust(e)).collect();
            let first_body: Vec<_> = first_case.stmts.iter().map(|s| go_stmt_to_rust(s)).collect();
            let mut chain = quote! { if #(#first_conds)&&* { #(#first_body);* } };

            // Subsequent cases become `else if`
            for case in switch.cases.iter().skip(1) {
                if case.exprs.is_empty() {
                    continue;
                }
                let conds: Vec<_> = case.exprs.iter().map(|e| go_to_rust(e)).collect();
                let body: Vec<_> = case.stmts.iter().map(|s| go_stmt_to_rust(s)).collect();
                chain.extend(quote! { else if #(#conds)&&* { #(#body);* } });
            }

            // Default body as final `else`
            if !switch.default_stmts.is_empty() {
                let default_body: Vec<_> = switch.default_stmts.iter()
                    .map(|s| go_stmt_to_rust(s))
                    .collect();
                chain.extend(quote! { else { #(#default_body);* } });
            }

            return chain;
        }

        // No cases, only default
        if !switch.default_stmts.is_empty() {
            let db: Vec<_> = switch.default_stmts.iter()
                .map(|s| go_stmt_to_rust(s))
                .collect();
            return quote! { #(#db);* };
        }
        quote! { () }
    } else {
        // Build selector
        let selector = switch.selector.as_ref()
            .map(|s| go_to_rust(s))
            .unwrap_or_else(|| quote! { () });

        crate::debug_println!("DEBUG: transpile_switch - arms before quote:");
        for (i, arm) in arms.iter().enumerate() {
            crate::debug_println!("DEBUG: transpile_switch - arm {}: {}", i, arm);
        }
        // Debug: collect arms into a single token stream
        let arms_as_tokens: proc_macro2::TokenStream = arms.iter().cloned().collect();
        crate::debug_println!("DEBUG: transpile_switch - arms_as_tokens: {}", arms_as_tokens);
        crate::debug_println!("DEBUG: transpile_switch - arms_as_tokens is_empty: {}", arms_as_tokens.is_empty());
        // Build match expression with arms - use quote! correctly
        let result = quote! {
            match #selector {
                #arms_as_tokens
            }
        };
        crate::debug_println!("DEBUG: transpile_switch - final result (len={}): {}", result.to_string().len(), result);
        crate::debug_println!("DEBUG: transpile_switch - arms count: {}", arms.len());
        crate::debug_println!("DEBUG: transpile_switch - result is_empty: {}", result.is_empty());
        result
    }
}
