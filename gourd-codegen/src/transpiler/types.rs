//! Go type name mapping to Rust equivalents.

use proc_macro2::TokenStream;
use quote::quote;

/// Map a single Go type identifier to its Rust equivalent.
pub(crate) fn go_type_map(ident: &syn::Ident) -> TokenStream {
    let name = ident.to_string();
    match name.as_str() {
        "bool"    => quote! { bool },
        "string"  => quote! { String },
        "int"     => quote! { i32 },
        "int8"    => quote! { i8 },
        "int16"   => quote! { i16 },
        "int32"   => quote! { i32 },
        "int64"   => quote! { i64 },
        "uint"    => quote! { u32 },
        "uint8"   => quote! { u8 },
        "uint16"  => quote! { u16 },
        "uint32"  => quote! { u32 },
        "uint64"  => quote! { u64 },
        "uintptr" => quote! { usize },
        "byte"    => quote! { u8 },
        "rune"    => quote! { char },
        "float32" => quote! { f32 },
        "float64" => quote! { f64 },
        "error"   => quote! { Box<dyn std::error::Error> },
        _         => quote! { #ident },
    }
}

/// Map Go type names to Rust, handling Path and composite types.
pub(crate) fn map_go_types(ty: &syn::Type) -> TokenStream {
    match ty {
        syn::Type::Path(type_path) => {
            if let Some(first) = type_path.path.segments.first() {
                let first_name = first.ident.to_string();
                if matches!(first_name.as_str(),
                    "bool" | "string" | "int" | "int8" | "int16" | "int32" | "int64"
                    | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr"
                    | "byte" | "rune" | "float32" | "float64" | "error"
                ) {
                    return go_type_map(&first.ident);
                }
            }
            quote! { #ty }
        }
        syn::Type::Reference(type_ref) => {
            let elem = map_go_types(&type_ref.elem);
            match &type_ref.lifetime {
                Some(l) => quote! { & #l #elem },
                None => quote! { &#elem },
            }
        }
        syn::Type::Slice(type_array) => {
            let elem = map_go_types(&type_array.elem);
            quote! { &[ #elem ] }
        }
        syn::Type::Array(a) => {
            let elem = map_go_types(&a.elem);
            quote! { [ #elem; #a.len ] }
        }
        syn::Type::Tuple(type_tuple) => {
            let elems: Vec<_> = type_tuple.elems.iter().map(map_go_types).collect();
            match elems.len() {
                1 => quote! { ( #(#elems),* ) },
                0 => quote! { () },
                _ => quote! { ( #(#elems),* ) },
            }
        }
        syn::Type::Paren(inner) => {
            let mapped = map_go_types(&inner.elem);
            quote! { ( #mapped ) }
        }
        _ => quote! { #ty },
    }
}
