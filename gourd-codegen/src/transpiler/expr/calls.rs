//! Call and method transpilation: `Call`, `MethodCall`, `Field`, `Index`.
//! Also handles Rust macros (`Expr::Macro`) and method-call shorthand.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, ExprField, ExprIndex, ExprMacro, ExprMethodCall};

use super::dispatch::emit_todo;

pub fn transpile_call(input: &syn::ExprCall) -> TokenStream {
    let args: Vec<_> = input.args.iter().map(super::dispatch::go_to_rust).collect();

    // Handle Go builtins that don't have direct Rust equivalents
    if let Expr::Path(path) = &*input.func {
        if let Some(name) = path.path.get_ident() {
            let name_str = name.to_string();
            
            // `copy(dst, src)` → dst.copy_from_slice(&src) returning len
            if name_str == "copy" {
                if input.args.len() != 2 {
                    return emit_todo("copy() requires exactly two arguments");
                }
                let dst = super::dispatch::go_to_rust(&input.args[0]);
                let src = super::dispatch::go_to_rust(&input.args[1]);
                return quote! { { #dst.copy_from_slice(&#src); #dst.len() } };
            }
            
            // `delete(m, key)` → m.remove(&key)
            if name_str == "delete" {
                if input.args.len() != 2 {
                    return emit_todo("delete() requires exactly two arguments");
                }
                let map = super::dispatch::go_to_rust(&input.args[0]);
                let key = super::dispatch::go_to_rust(&input.args[1]);
                return quote! { #map.remove(& #key) };
            }
        }
    }

    if let Expr::Path(path) = &*input.func
        && let Some(name) = path.path.get_ident()
        && matches!(name.to_string().as_str(), "len" | "cap")
    {
        let arg = args[0].clone();
        return quote! { #arg.len() as i32 };
    }
    // Go type conversion calls: int(), int8(), ..., string(), bool(), byte(), rune(), ...
    if let Expr::Path(path) = &*input.func
        && let Some(name) = path.path.get_ident()
    {
        let name_str = name.to_string();
        match name_str.as_str() {
            // Type conversions: `int(x)` → `(x as i32)`, etc.
            "int" | "int8" | "int16" | "int32" | "int64"
            | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr" => {
                let rust_cast_str = match name_str.as_str() {
                    "int" => "i32",
                    "int8" => "i8",
                    "int16" => "i16",
                    "int32" => "i32",
                    "int64" => "i64",
                    "uint" => "u32",
                    "uint8" => "u8",
                    "uint16" => "u16",
                    "uint32" => "u32",
                    "uint64" => "u64",
                    "uintptr" => "usize",
                    _ => unreachable!(),
                };
                let rust_cast: syn::Ident = syn::parse_str(rust_cast_str).unwrap();
                return quote! { (#(#args),* as #rust_cast) };
            }
            "float32" => {
                return quote! { (#(#args),* as f32) };
            }
            "float64" => {
                return quote! { (#(#args),* as f64) };
            }
            "bool" => {
                return quote! { (#(#args),* as bool) };
            }
            "string" => {
                let arg = args[0].clone();
                // string(bytes) → from_utf8(...)
                // string(rune) → String::from(char to string)
                return quote! { std::str::from_utf8(&#arg).unwrap_or("").to_string() };
            }
            "byte" => {
                return quote! { (#(#args),* as u8) };
            }
            "rune" => {
                return quote! { (#(#args),* as char) };
            }
            _ => {}
        }
    }
    // Go `new` builtin: `new(Foo)` → `Foo::default()`
    // Maps Go primitive types to Rust equivalents (int → i32, etc.).
    if let Expr::Path(path) = &*input.func
        && let Some(name) = path.path.get_ident()
        && name.to_string() == "new"
    {
        if input.args.len() == 1 {
            let arg = &input.args[0];
            // For type names (paths), map Go type → Rust type and emit ::default()
            if let Expr::Path(arg_path) = arg {
                let type_str = quote! { #arg_path }.to_string();
                // Map Go primitive type names to Rust equivalents
                let mapped_str = match type_str.as_str() {
                    "int" => "i32",
                    "int8" => "i8",
                    "int16" => "i16",
                    "int32" => "i32",
                    "int64" => "i64",
                    "uint" => "u32",
                    "uint8" => "u8",
                    "uint16" => "u16",
                    "uint32" => "u32",
                    "uint64" => "u64",
                    "uintptr" => "usize",
                    "byte" => "u8",
                    "rune" => "char",
                    "float32" => "f32",
                    "float64" => "f64",
                    "string" => "String",
                    "bool" => "bool",
                    "error" => "Box<dyn std::error::Error>",
                    _ => &type_str, // user-defined type, keep as-is
                };
                if let Ok(mapped_ty) = syn::parse_str::<syn::Type>(mapped_str) {
                    return quote! { #mapped_ty::default() };
                }
                // For user-defined types (structs, etc.), just emit ::default()
                return quote! { #arg::default() };
            } else {
                // Could not extract a type — emit compile_error!
                return emit_todo("new() requires a type argument");
            }
        } else {
            return emit_todo("new() requires exactly one type argument");
        }
    }
    // Go `panic` builtin: `panic("msg")` → `panic!("msg")`
    // String literals must remain as raw literals (not String::from(...)).
    if let Expr::Path(path) = &*input.func
        && let Some(name) = path.path.get_ident()
        && name.to_string() == "panic"
    {
        if input.args.is_empty() {
            return quote! { panic!("panic()") };
        }
        // For panic!, pass string literals directly rather than String::from(...)
        let panic_args: Vec<_> = input.args.iter().map(|arg| {
            if let Expr::Lit(lit) = arg
                && matches!(&lit.lit, syn::Lit::Str(_))
            {
                // Pass the string literal directly for panic! format string
                quote! { #arg }
            } else {
                // Non-string args: use the transpiled expression
                let transpiled = super::dispatch::go_to_rust(arg);
                quote! { #transpiled }
            }
        }).collect();
        return quote! { panic!( #(#panic_args),* ) };
    }
    // Go `append` builtin: `append(slice, items...)` → push each item.
    // Go's append is variadic: `append(slice)` (no-op),
    // `append(slice, x)` (push one), `append(slice, x, y, z)` (push many).
    // Works with slice literals: `append([]int{1, 2}, 3)` → `vec![1, 2, 3]`
    // and variable slices: `append(data, x)` → pushes x to a copy of data.
    if let Expr::Path(path) = &*input.func
        && let Some(name) = path.path.get_ident()
        && name.to_string() == "append"
    {
        let append_args: Vec<_> = input.args.iter().collect();
        if append_args.is_empty() {
            return emit_todo("append() requires at least one argument");
        }
        let slice = super::dispatch::go_to_rust(&append_args[0]);
        if append_args.len() == 1 {
            // append(slice) — no-op, just return the slice
            return quote! { #slice };
        }
        // append(slice, items...) — push each item individually
        let items: Vec<_> = append_args[1..].iter().map(|arg| {
            let item = super::dispatch::go_to_rust(arg);
            quote! { __gourd_append_result.push(#item); }
        }).collect();
        // Convert slice to Vec, push each item, return the Vec
        return quote! {
            {
                let mut __gourd_append_result = #slice.to_vec();
                #(#items)*
                __gourd_append_result
            }
        };
    }
    // Go `make` builtin — special handling for chan/map/slice types.
    // `make(chan T, cap)` → `GoChannel::<T>::with_capacity(cap)`
    // `make(chan T)` → `GoChannel::<T>::new()`
    // `make(map[K]V)` → `HashMap::new()`
    // `make([]T, len)` → `vec![0; len]`
    if let Expr::Path(path) = &*input.func
        && let Some(name) = path.path.get_ident()
        && name.to_string() == "make"
    {
        let make_args: Vec<_> = input.args.iter().collect();
        if make_args.len() >= 2 {
            let type_expr = &make_args[0];
            let type_tokens = super::dispatch::go_to_rust(type_expr);
            let type_str = quote! { #type_expr }.to_string();

            // Determine if this is a channel, map, or slice type.
            // Channel types use the `chan T` marker (either `chan` or `__go_chan`).
            // Map types use `map[K]V` syntax.
            // Slice types use `[]T` syntax.
            if type_str.contains("chan") || type_str.contains("__go_chan") {
                // Channel: make(chan T) or make(chan T, cap)
                if make_args.len() == 2 {
                    // Unbuffered: make(chan T) → GoChannel::<T>::new()
                    quote! { GoChannel::<#type_tokens>::new() }
                } else {
                    // Buffered: make(chan T, cap) → GoChannel::<T>::with_capacity(cap)
                    let cap = super::dispatch::go_to_rust(&make_args[1]);
                    quote! { GoChannel::<#type_tokens>::with_capacity(#cap) }
                }
            } else if type_str.contains("map[") {
                // Map: make(map[K]V) → HashMap::new()
                quote! { ::std::collections::HashMap::new() }
            } else if type_str.starts_with("[]") {
                // Slice: make([]T, len) → vec![0; len]
                // Need to use default() for the repeat value since type_tokens is a type
                let type_default: TokenStream = quote! { #type_tokens::default() };
                if make_args.len() == 2 {
                    let len = super::dispatch::go_to_rust(&make_args[1]);
                    quote! { ::std::iter::repeat(#type_default).take(#len).collect::<#type_tokens>() }
                } else {
                    // make([]T, len, cap) — cap is ignored, same as len
                    let len = super::dispatch::go_to_rust(&make_args[1]);
                    quote! { ::std::iter::repeat(#type_default).take(#len).collect::<#type_tokens>() }
                }
            } else {
                // Unknown type — emit compile_error!
                emit_todo("unsupported type in make()")
            }
        } else {
            emit_todo("make() requires at least a type argument")
        }
    } else {
        let func = super::dispatch::go_to_rust(&input.func);
        quote! { #func( #(#args),* ) }
    }
}

/// Handle Rust macro invocations (e.g. `vec![...]`) passed through `quote!`.
/// These are valid Rust already — just emit the macro tokens as-is.
pub fn go_to_rust_macro(input: &ExprMacro) -> TokenStream {
    quote! { #input }
}

pub fn transpile_index(input: &ExprIndex) -> TokenStream {
    let seq = super::dispatch::go_to_rust(&input.expr);
    let idx = super::dispatch::go_to_rust(&input.index);
    // Check if the index is a string literal — for maps, use .get() instead of direct indexing
    if let Expr::Lit(lit) = &*input.index
        && matches!(&lit.lit, syn::Lit::Str(_))
    {
        // Map lookup: m["key"] → m.get(&"key").unwrap()
        return quote! { #seq.get(& #idx).unwrap() };
    }
    // Index expressions need usize for Rust slices; if the index is i32 (Go int),
    // cast it to usize automatically.
    let idx = quote! { #idx as usize };
    quote! { #seq[ #idx ] }
}

pub fn transpile_method_call(input: &ExprMethodCall) -> TokenStream {
    let receiver = super::dispatch::go_to_rust(&input.receiver);
    let method_name = &input.method;
    let args: Vec<_> = input.args.iter().map(super::dispatch::go_to_rust).collect();
    if method_name.to_string() == "get" {
        if let Some(first) = args.first() {
            let rest: Vec<_> = args.iter().skip(1).cloned().collect();
            return quote! { #receiver.#method_name( &#first #(#rest),* ) };
        }
    }
    quote! { #receiver.#method_name( #(#args),* ) }
}

pub fn transpile_field(input: &ExprField) -> TokenStream {
    let base = super::dispatch::go_to_rust(&input.base);
    let field = &input.member;
    quote! { #base.#field }
}
