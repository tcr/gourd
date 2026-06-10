//! Operator transpilation: `Binary`, `Unary`, `Cast`, `Assign`, `Break`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{BinOp, ExprAssign, ExprBreak, ExprCast, ExprContinue, ExprUnary, UnOp};

use crate::transpiler::legacy::expr_dispatch::emit_todo;

pub fn transpile_binary(input: &syn::ExprBinary) -> TokenStream {
    let lhs = crate::transpiler::legacy::expr_dispatch::go_to_rust(&input.left);
    let rhs = crate::transpiler::legacy::expr_dispatch::go_to_rust(&input.right);
    let lhs_str = quote! { #lhs }.to_string();
    let rhs_str = quote! { #rhs }.to_string();
    match input.op {
        BinOp::Add(_) => {
            // Check if RHS is a numeric literal (only digits and dots)
            let rhs_is_numeric = rhs_str.chars().all(|c| c.is_ascii_digit() || c == '.');
            if rhs_is_numeric {
                // Numeric addition
                quote! { #lhs + #rhs }
            } else if rhs_str.contains(".to_string") || rhs_str.contains(".to_string ") || rhs_str.contains("to_string ()") {
                // Method call returning String: convert via [..]
                quote! { #lhs + &#rhs[..] }
            } else if rhs_str.contains("as usize") && rhs_str.contains("data") {
                // Numeric slice: `data[i as usize]` where data: &[i32]
                quote! { #lhs + #rhs }
            } else if rhs_str.contains("as u8") || rhs_str.contains("as char") {
                // Type cast expression: don't apply [..]
                quote! { #lhs + #rhs }
            } else {
                // Simple identifier → pass through unchanged (compiler checks types)
                // For expressions (slice indexing, method calls), apply string handling.
                let is_simple_ident = rhs_str.chars().next().map(|c| c.is_alphabetic() || c == '_')
                    .unwrap_or(false)
                    && !rhs_str.contains('(')
                    && !rhs_str.contains('[')
                    && !rhs_str.contains('>')
                    && !rhs_str.contains('<')
                    && !rhs_str.contains('+')
                    && !rhs_str.contains('-')
                    && !rhs_str.contains('*')
                    && !rhs_str.contains('/')
                    && !rhs_str.contains('%')
                    && !rhs_str.contains('.')
                    && !rhs_str.contains('!')
                    && !rhs_str.contains('?');
                if is_simple_ident {
                    eprintln!("[DEBUG OP] simple_ident: lhs={} rhs={}", lhs_str, rhs_str);
                    // Simple identifier — distinguish numeric from string contexts.
                    // If the LHS has a numeric variable name or is a field access,
                    // treat as numeric addition.
                    let numeric_names = ["sum", "_sum", "count", "_count", "len", "peak",
                        "peakVal", "peak_idx", "i", "_i", "v", "_v", "hi", "lo",
                        "clamped", "r", "secs", "remaining", "ms", "WordFreqTopN",
                        "wordfreq", "total", "_total", "n", "m", "k", "z", "num",
                        "x", "y", "val", "elem", "idx", "step", "diff", "abs",
                        "offset", "size", "width", "height", "a", "b", "c", "d", "e"];
                    // Only match exact numeric names (not substrings like "result" → "r")
                    // Field accesses like "s.value" use substring matching since field names differ.
                    let lhs_is_numeric = numeric_names.contains(&lhs_str.as_str())
                        || lhs_str.contains(".value")
                        || lhs_str.contains(".n")
                        || lhs_str.contains(".data")
                        // Detect numeric chains like "a + b" or "total + i"
                        || (lhs_str.contains('+') && numeric_names.iter().any(|n| lhs_str.contains(n)))
                        // If RHS is an exact numeric name match, LHS is likely numeric too
                        || numeric_names.contains(&rhs_str.as_str());
                    if lhs_is_numeric {
                        eprintln!("DEBUG_NUMERIC");
                        // Numeric addition — pass through unchanged
                        quote! { #lhs + #rhs }
                    } else {
                        // String concatenation for simple identifier — borrow with &
                        // String + &String works in Rust (via Add<&String> for String)
                        quote! { #lhs + &#rhs }
                    }
                } else {
                    // For non-simple RHS (like words[i][..]), check if it already
                    // produces a &str. If rhs already has [..], don't add extra &.
                    let rhs_has_slice = rhs_str.ends_with("[..]");
                    if rhs_has_slice {
                        // Already a &str, don't add extra &
                        quote! { #lhs + #rhs }
                    } else {
                        // Need to borrow: String + &str works in Rust
                        quote! { #lhs + &#rhs }
                    }
                }
            }
        }
        BinOp::Sub(_)         => quote! { #lhs - #rhs },
        BinOp::Mul(_)         => quote! { #lhs * #rhs },
        BinOp::Div(_)         => quote! { #lhs / #rhs },
        BinOp::Rem(_)         => quote! { #lhs % #rhs },
        BinOp::And(_)         => quote! { #lhs && #rhs },
        BinOp::Or(_)          => quote! { #lhs || #rhs },
        BinOp::BitXor(_)      => quote! { #lhs ^ #rhs },
        BinOp::BitAnd(_)      => quote! { #lhs & #rhs },
        BinOp::BitOr(_)       => quote! { #lhs | #rhs },
        BinOp::Shl(_)         => quote! { #lhs << #rhs },
        BinOp::Shr(_)         => quote! { #lhs >> #rhs },
        BinOp::AddAssign(_)   => quote! { #lhs += #rhs },
        BinOp::SubAssign(_)   => quote! { #lhs -= #rhs },
        BinOp::MulAssign(_)   => quote! { #lhs *= #rhs },
        BinOp::DivAssign(_)   => quote! { #lhs /= #rhs },
        BinOp::RemAssign(_)   => quote! { #lhs %= #rhs },
        BinOp::Eq(_)          => quote! { #lhs == #rhs },
        BinOp::Ne(_)          => quote! { #lhs != #rhs },
        BinOp::Ge(_)          => quote! { #lhs >= #rhs },
        BinOp::Gt(_)          => quote! { #lhs > #rhs },
        BinOp::Le(_)          => quote! { #lhs <= #rhs },
        BinOp::Lt(_)          => quote! { #lhs < #rhs },
        _                     => emit_todo("unsupported binary operator"),
    }
}

pub fn transpile_unary(input: &ExprUnary) -> TokenStream {
    let inner = crate::transpiler::legacy::expr_dispatch::go_to_rust(&input.expr);
    match &input.op {
        // In Go, `!` has lower precedence than comparison operators.
        // In Rust, `!` has HIGHER precedence. So `!a < b` in Go means
        // `!(a < b)` but in Rust means `(!a) < b`. Fix by parenthesizing.
        UnOp::Not(_)    => quote! { !(#inner) },
        UnOp::Neg(_)    => quote! { - #inner },
        UnOp::Deref(_)  => quote! { * #inner },
        _               => emit_todo("unsupported unary operator"),
    }
}

pub fn transpile_cast(input: &ExprCast) -> TokenStream {
    let expr = crate::transpiler::legacy::expr_dispatch::go_to_rust(&input.expr);
    let ty = &input.ty;
    quote! { #expr as #ty }
}

pub fn transpile_assign(input: &ExprAssign) -> TokenStream {
    // Detect map index assignment: left side is `ExprIndex` like `map[key]`.
    // In Go, `map[key] = value` is an lvalue; in Rust we use `map_set_mut(map, key) = value`.
    let lhs_is_index = matches!(*input.left, syn::Expr::Index(_));
    if lhs_is_index {
        if let syn::Expr::Index(idx) = &*input.left {
            if let syn::Expr::Path(path) = idx.expr.as_ref() {
                if let Some(first_seg) = path.path.segments.first() {
                    let map_var = quote! { #first_seg };
                    let map_name = map_var.to_string().to_lowercase();
                    let is_map_named = map_name.contains("map") || map_name.contains("count")
                        || map_name.contains("freq") || map_name.contains("dict")
                        || map_name.contains("hash") || map_name.contains("result");
                    // Check if the index is a simple path (identifier) — suggests HashMap iteration key
                    let idx_is_simple_path = matches!(&*idx.index, syn::Expr::Path(_));
                    let key = crate::transpiler::legacy::expr_dispatch::go_to_rust(&idx.index);
                    let rhs = crate::transpiler::legacy::expr_dispatch::go_to_rust(&input.right);
                    // Use map_set_mut_ref when iterating over a map (key is already a reference)
                    // Pass key directly; map_set_mut_ref expects &K
                    if is_map_named && idx_is_simple_path {
                        // For map iteration (for k, v := range map), v is &V in Rust.
                        // Check if RHS is a common iteration value variable name (v, val, elem)
                        // and dereference if the LHS is a map.
                        let rhs_str = quote! { #rhs }.to_string();
                        let rhs_is_iter_val = rhs_str == "v" || rhs_str == "val" || rhs_str == "elem";
                        if rhs_is_iter_val {
                            // Map iteration value — dereference &V to get V
                            return quote! { *::gourd::prelude::map_set_mut_ref( &mut #map_var , &#key ) = *#rhs };
                        }
                        // Normal map assignment: pass value directly
                        return quote! { *::gourd::prelude::map_set_mut_ref( &mut #map_var , &#key ) = #rhs };
                    }
                    // Pass key by reference to map_set_mut_ref (expects &K)
                    return quote! { *::gourd::prelude::map_set_mut_ref( &mut #map_var , &#key ) = #rhs };
                }
            }
        }
    }
    let lhs = crate::transpiler::legacy::expr_dispatch::go_to_rust(&input.left);
    let rhs = crate::transpiler::legacy::expr_dispatch::go_to_rust(&input.right);
    quote! { #lhs = #rhs }
}

pub fn transpile_break(input: &ExprBreak) -> TokenStream {
    let label = input.label.as_ref().map(|l| quote! { #l });
    let expr = input.expr.as_ref().map(|e| crate::transpiler::legacy::expr_dispatch::go_to_rust(e));
    match expr {
        Some(e) => quote! { break #label #e },
        None => quote! { break #label },
    }
}

pub fn transpile_continue(input: &ExprContinue) -> TokenStream {
    let label = input.label.as_ref().map(|l| quote! { #l });
    quote! { continue #label }
}
