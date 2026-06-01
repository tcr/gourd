//! Call and method transpilation: `Call`, `MethodCall`, `Field`, `Index`.
//! Also handles Rust macros (`Expr::Macro`) and method-call shorthand.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, ExprField, ExprIndex, ExprMacro, ExprMethodCall};

pub fn transpile_call(input: &syn::ExprCall) -> TokenStream {
    let args: Vec<_> = input.args.iter().map(super::dispatch::go_to_rust).collect();
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
    let func = super::dispatch::go_to_rust(&input.func);
    quote! { #func( #(#args),* ) }
}

/// Handle Rust macro invocations (e.g. `vec![...]`) passed through `quote!`.
/// These are valid Rust already — just emit the macro tokens as-is.
pub fn go_to_rust_macro(input: &ExprMacro) -> TokenStream {
    quote! { #input }
}

pub fn transpile_index(input: &ExprIndex) -> TokenStream {
    let seq = super::dispatch::go_to_rust(&input.expr);
    let idx = super::dispatch::go_to_rust(&input.index);
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
