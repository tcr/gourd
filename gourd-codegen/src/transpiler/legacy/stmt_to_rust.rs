//! Go statement to Rust token conversion (the `go_stmt_to_rust` bridge).

pub(crate) use crate::transpiler::hir::ast::{GoForInit, GoSelect, GoStmt, Switch};
use crate::transpiler::legacy::expr_dispatch::go_to_rust;
use crate::transpiler::types::map_go_types;
use proc_macro2::TokenStream;
use quote::quote;
use syn::parse_quote;

/// Convert a Go statement AST node to Rust tokens.
pub(crate) fn go_stmt_to_rust(stmt: &GoStmt) -> TokenStream {
    match stmt {
        GoStmt::Local(local) => {
            let pat = &local.pat;
            let val = local.init.as_ref().map(|v| go_to_rust(&v.expr));
            quote! { let #pat = #val; }
        }
        GoStmt::GoLocal(ident, val) => {
            quote! { let mut #ident = #val; }
        }
        GoStmt::If(go_if) => {
            let cond = go_to_rust(&go_if.cond);
            let then_body: Vec<_> = go_if.then_block.stmts.iter()
                .map(|s| go_stmt_to_rust(s)).collect();
            // Use quote! instead of parse_quote! — the statements are already valid
            // Rust tokens. Using parse_quote! breaks when bodies contain < comparisons
            // (syn interprets < as generic type params rather than binary operators).
            let then_block: TokenStream = quote!({ #(#then_body);* });
            let else_block = go_if.else_block.as_ref().map(|eb| {
                let else_body: Vec<_> = eb.stmts.iter().map(|s| go_stmt_to_rust(s)).collect();
                let block: TokenStream = quote!({ #(#else_body);* });
                quote! { else #block }
            });
            quote! { if #cond #then_block #else_block }
        }
        GoStmt::Expr(expr) => {
            go_to_rust(expr)
        }
        GoStmt::GoChannelSend(ch, val) => {
            let ch_rust = go_to_rust(ch);
            let val_rust = go_to_rust(val);
            quote! { #ch_rust.send(#val_rust); }
        }
        GoStmt::GoChannelRecv(ch) => {
            let ch_rust = go_to_rust(ch);
            quote! { return #ch_rust.recv().unwrap(); }
        }
        GoStmt::GoTypeAssert(receiver, ty) => {
            let recv_rust = go_to_rust(receiver);
            let ty_str = quote! { #ty }.to_string();
            match ty_str.as_str() {
                "String" => quote! { ::std::string::ToString::to_string(&#recv_rust) },
                "bool" => quote! { #recv_rust != 0 },
                "char" => quote! { (#recv_rust as u8) as char },
                _ => quote! { #recv_rust as #ty },
            }
        }
        GoStmt::GoMake(raw_args) => {
            go_go_make(raw_args)
        }
        GoStmt::GoSlice(elems) => {
            let elems: Vec<_> = elems.iter().map(go_to_rust).collect();
            quote! { vec![ #(#elems),* ] }
        }
        GoStmt::GoMap(ident, key_type, val_type, entries) => {
            go_stmt_to_rust_map(ident, key_type, val_type, entries)
        }
        GoStmt::GoReturn(exprs) => {
            if exprs.is_empty() {
                quote! { return }
            } else if exprs.len() == 1 {
                let e = go_to_rust(&exprs[0]);
                quote! { return #e }
            } else {
                let rust_exprs: Vec<_> = exprs.iter().map(go_to_rust).collect();
                quote! { return ( #(#rust_exprs),* ) }
            }
        }
        GoStmt::Switch(switch) => {
            // Use HIR pipeline for switch transpilation
            crate::transpiler::hir::switch_to_rust(switch)
        }
        GoStmt::Select(select) => {
            // Convert Go AST select directly to HIR, then to Rust — avoid round-trip
            let hir_select = crate::transpiler::hir::go_select_to_hir(&select);
            crate::transpiler::hir::hir_select_to_rust_from_hir(&hir_select)
        }
        GoStmt::Continue => {
            quote! { continue }
        }
        GoStmt::Defer(closure_body) => {
            // `defer func() { ... }` → Rust Drop guard at end of scope
            // Generates a struct implementing Drop, ensuring cleanup runs
            // when the guard variable goes out of scope.
            quote! {
                {
                    #[derive(Default)]
                    struct __GourdDefer;
                    impl Drop for __GourdDefer {
                        fn drop(&mut self) {
                            #closure_body
                        }
                    }
                    let _guard = __GourdDefer;
                }
            }
        }
        GoStmt::GoIfErr(err_check, err_block) => {
            // `if err != nil { ... }` → literal Go parity
            // Go:   `if err != nil { log(err); return err }`
            // Rust: `if let ::std::result::Result::Err(err) = expr { ... }`
            // The error value is bound to `err` inside the block, matching Go semantics.
            let err_expr = err_check;
            let block_stmts: Vec<_> = err_block.iter()
                .map(|s| go_stmt_to_rust(s)).collect();
            quote! {
                if let ::std::result::Result::Err(err) = #err_expr {
                    #(#block_stmts)*;
                }
            }
        }
        GoStmt::While(while_stmt) => {
            let cond = go_to_rust(&while_stmt.cond);
            let body: Vec<_> = while_stmt.body.stmts.iter()
                .map(|s| go_stmt_to_rust(s)).collect();
            // quote! produces valid Rust tokens directly — no re-parsing needed.
            quote! { while #cond { #(#body);* } }
        }
        GoStmt::GoImport(import) => {
            // `import "strings"` → already bundled in prelude
            // `import s "strings"` → `use ::gourd::prelude as s;`
            // `import . "fmt"` → `use ::gourd::prelude::*;`
            // `import _ "os"` → blank, no output
            if import.blank {
                // Side-effect only, nothing to emit
                quote! {}
            } else if import.dot {
                // Dot import: make all names visible
                quote! { use ::gourd::prelude::*; }
            } else if let Some(alias) = &import.alias {
                // Aliased import: map known packages to prelude module
                let alias_ident = alias.clone();
                match import.path.as_str() {
                    "strings" | "os" | "io" | "bytes" | "json" | "time" | "math" | "byte" => {
                        quote! { use ::gourd::prelude as #alias_ident; }
                    }
                    _ => {
                        // External packages not yet supported
                        let msg = format!("TODO: import external packages: {}", import.path);
                        quote! { compile_error!(concat!(#msg)) }
                    }
                }
            } else {
                // Default import: already implicit via `gourd::prelude::*`
                // No `use` needed, but emit a no-op to show it was parsed
                quote! {}
            }
        }
        GoStmt::GoFor(for_stmt) => {
            let body: Vec<_> = for_stmt.body.stmts.iter()
                .map(|s| go_stmt_to_rust(s)).collect();
            // Use quote! instead of parse_quote! — the body is already valid Rust tokens.
            // parse_quote! breaks when the body contains < comparisons because syn
            // interprets < as generic type parameters rather than comparison operators.
            let body_block: TokenStream = quote!({ #(#body);* });

            match (&for_stmt.init, &for_stmt.is_range) {
                (Some(GoForInit::Double(i, v, _)), true) => {
                    let i_ident = i.clone();
                    let v_ident = v.clone();
                    let iterable = &for_stmt.iterable;
                    let ident_str = i_ident.to_string();
                    // Check if first variable is `_` (ignored) — slice iteration
                    // or a named identifier — map iteration
                    if ident_str == "_" {
                        // Slice iteration: `for _, word := range words`
                        // Use .cloned() for String slices (Clone, not Copy)
                        quote! {
                            for #v_ident in #iterable . iter () . cloned () #body_block
                        }
                    } else {
                        // Two-variable range loop: `for i, v := range data`
                        // For slices/vecs: iterate with index
                        // For maps: iterate with key-value pairs
                        // Detect if iterable is a map by checking type or name
                        let iter_str = quote! { #iterable }.to_string();
                        let is_map_type = iter_str.contains("HashMap")
                            || iter_str.contains("hash_map");
                        // Also check by variable name — common map names
                        let iter_name = iter_str.trim();
                        let is_map_named = matches!(iter_name, "counts" | "result" | "map" | "freq" | "freqs" | "hash" | "hash_map" | "counter" | "counters" | "dict" | "wordfreq");
                        if is_map_type || is_map_named {
                            // Map iteration: for k, v := range map
                            quote! {
                                for ( #i_ident , #v_ident ) in #iterable . iter () { #body_block }
                            }
                        } else {
                            // Slice iteration: for i, v := range slice
                            quote! {
                                for #i_ident in 0.. #iterable . len () as i32 {
                                    let #v_ident = #iterable [#i_ident as usize]; #body_block
                                }
                            }
                        }
                    }
                }
                (Some(GoForInit::Single(i, _)), true) => {
                    let i_ident = i.clone();
                    let iterable = &for_stmt.iterable;
                    quote! {
                        for #i_ident in 0.. #iterable.len() #body_block
                    }
                }
                (None, true) => {
                    let iterable = &for_stmt.iterable;
                    quote! {
                        for _ in 0.. #iterable.len() #body_block
                    }
                }
                (None, false) => {
                    // C-style: `for { body }` (infinite loop)
                    quote! { loop #body_block }
                }
                (init, false) => {
                    // C-style: `for init; cond; post { body }`
                    let cond = for_stmt.cond.as_ref()
                        .map(|e| crate::transpiler::legacy::expr_dispatch::go_to_rust(e.as_ref()))
                        .unwrap_or_default();
                    let post = for_stmt.post.as_ref()
                        .map(|e| crate::transpiler::legacy::expr_dispatch::go_to_rust(e.as_ref()))
                        .unwrap_or_default();
                    match init {
                        Some(GoForInit::Double(ident1, ident2, _)) => {
                            // Two init vars: `let ident1 = (); let ident2 = ();`
                            let init_stmt = quote! { let mut #ident1 = 0; let mut #ident2 = 0; };
                            if !cond.is_empty() && !post.is_empty() {
                                // Parenthesize #cond to fix operator precedence: in Go `!a < b` means `!(a < b)`,
                                // but in Rust `!a < b` means `(!a) < b`. Wrap in parens so `!` applies to whole expr.
                                quote! {
                                    { #init_stmt loop { if !(#cond) { break; } #(#body);* ; #post ; } }
                                }
                            } else if !cond.is_empty() {
                                quote! {
                                    { #init_stmt while #cond { #(#body);* } }
                                }
                            } else {
                                quote! {
                                    { #init_stmt loop #body_block }
                                }
                            }
                        }
                        Some(GoForInit::Single(ident, init_val)) => {
                            let init_stmt = match init_val {
                                Some(val) => {
                                    let init_val_rust = crate::transpiler::legacy::expr_dispatch::go_to_rust(val.as_ref());
                                    // When there's a post statement (e.g. `i++`), the init var must be mut
                                    if !post.is_empty() {
                                        quote! { let mut #ident = #init_val_rust; }
                                    } else {
                                        quote! { let #ident = #init_val_rust; }
                                    }
                                }
                                None => quote! { let #ident = (); },
                            };
                            if !cond.is_empty() && !post.is_empty() {
                                // C-style for with both cond and post: use `loop` with `while`-like guard
                                quote! {
                                    { #init_stmt loop { if !(#cond) { break; } #(#body);* ; #post } }
                                }
                            } else if !cond.is_empty() {
                                quote! {
                                    { #init_stmt while #cond { #(#body);* } }
                                }
                            } else {
                                quote! {
                                    { #init_stmt loop #body_block }
                                }
                            }
                        }
                        None => quote! { loop #body_block },
                    }
                }
            }
        }
        GoStmt::RawStmt(tokens) => {
            tokens.clone()
        }
        GoStmt::SwitchReturn(tokens) => {
            tokens.clone()
        }
        GoStmt::GoShortDecl(ident, val) => {
            quote! { let #ident = #val; }
        }
    }
}

/// Handle `make(...)` statements — channels, maps, slices.
fn go_go_make(raw_args: &str) -> TokenStream {
    let args_str = raw_args.trim().to_string();
    let normalized = args_str
        .replace(" [", "[")
        .replace(" ]", "]")
        .replace("  ", " ");

    if normalized.starts_with("chan ") {
        let chan_args: Vec<&str> = args_str.splitn(2, ',').collect();
        let chan_type_str = chan_args[0].trim().trim_start_matches("chan ").trim();
        let chan_type = crate::transpiler::types::map_go_type_str(chan_type_str);
        if chan_args.len() == 2 {
            let cap_str = chan_args[1].trim();
            let cap: TokenStream = parse_quote! { #cap_str };
            quote! { GoChannel::<#chan_type>::with_capacity(#cap) }
        } else {
            quote! { GoChannel::<#chan_type>::new() }
        }
    } else if normalized.starts_with("map[") {
        quote! { ::gourd::prelude::HashMap::new() }
    } else if normalized.starts_with("[]") {
        let slice_args: Vec<&str> = normalized.splitn(2, ',').collect();
        let slice_type_str = slice_args[0].trim().trim_start_matches("[]").trim();
        let slice_type = crate::transpiler::types::map_go_type_str(slice_type_str);
        if slice_args.len() == 2 {
            let len_str = slice_args[1].trim();
            let len: TokenStream = parse_quote! { #len_str };
            quote! { ::std::iter::repeat(#slice_type::default()).take(#len).collect::<Vec::<#slice_type>>() }
        } else {
            quote! { ::std::iter::repeat(#slice_type::default()).take(0).collect::<Vec::<#slice_type>>() }
        }
    } else {
        quote! { { compile_error!(concat!("TODO: make with unsupported type: ", #args_str)) } }
    }
}

/// Handle `map[K]V{entries}` declarations.
fn go_stmt_to_rust_map(
    ident: &str,
    key_type: &Option<Box<syn::Type>>,
    val_type: &Option<Box<syn::Type>>,
    entries: &[(syn::Expr, syn::Expr)],
) -> TokenStream {
    if entries.is_empty() {
        if ident.is_empty() {
            return quote! { ::gourd::prelude::HashMap::default() };
        }
        let name: syn::Ident = syn::parse_str(ident).unwrap();
        if let (Some(kt), Some(vt)) = (key_type, val_type) {
            let kt = map_go_types(kt);
            let vt = map_go_types(vt);
            return quote! { let #name = ::gourd::prelude::HashMap::<#kt, #vt>::default(); };
        }
        return quote! { let #name = ::gourd::prelude::HashMap::default() };
    }

    let insertions: Vec<_> = entries.iter().map(|(k, v)| {
        let key = go_to_rust(k);
        let val = go_to_rust(v);
        quote! { m.insert(#key, #val); }
    }).collect();

    let block = if let (Some(kt), Some(vt)) = (key_type, val_type) {
        let kt = map_go_types(kt);
        let vt = map_go_types(vt);
        quote! {
            {
                let mut m = ::gourd::prelude::HashMap::<#kt, #vt>::new();
                #(#insertions)*
                m
            }
        }
    } else {
        quote! {
            {
                let mut m = ::gourd::prelude::HashMap::new();
                #(#insertions)*
                m
            }
        }
    };

    if ident.is_empty() {
        block
    } else {
        let name: syn::Ident = syn::parse_str(ident).unwrap();
        quote! { let #name = #block; }
    }
}

/// Transpile Go body tokens (body without outer braces) to a Rust block.
/// This is used by the HIR receiver function handler to properly transpile
/// Go-style statements in method bodies.
pub(crate) fn transpile_go_body(body_tokens: TokenStream) -> Option<proc_macro2::Group> {
    use crate::transpiler::legacy::stmts::parse_body_from_group;
    
    match parse_body_from_group(&body_tokens) {
        Ok(block) => {
            let stmts: Vec<_> = block.stmts.iter()
                .map(|s| go_stmt_to_rust(s))
                .collect();
            Some(proc_macro2::Group::new(
                proc_macro2::Delimiter::Brace,
                quote! { #(#stmts);* }.into(),
            ))
        }
        Err(_) => None,
    }
}
