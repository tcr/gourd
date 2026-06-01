//! Go type name mapping to Rust equivalents.

/// Map a single Go type identifier to its Rust equivalent.
/// Returns a `syn::Type` so that generic parameters can be recursed into.
pub(crate) fn map_go_types(ty: &syn::Type) -> syn::Type {
    match ty {
        syn::Type::Path(type_path) => {
            // Check if the entire type is a single Go type identifier
            if type_path.path.segments.len() == 1 {
                let first_name = type_path.path.segments.first().unwrap().ident.to_string();
                if matches!(first_name.as_str(),
                    "bool" | "string" | "int" | "int8" | "int16" | "int32" | "int64"
                    | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr"
                    | "byte" | "rune" | "float32" | "float64" | "error"
                ) {
                    // Replace with the mapped Go type
                    let mapped_ident = match first_name.as_str() {
                        "bool" => "bool",
                        "string" => "String",
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
                        "error" => "Box",
                        _ => unreachable!(),
                    };
                    return syn::Type::Path(syn::TypePath {
                        path: syn::Path::from(syn::Ident::new(mapped_ident, proc_macro2::Span::call_site())),
                        qself: None,
                    });
                }
            }

            // Handle generic types like `Vec<int>` by recursing into generic arguments
            let mut new_segments = type_path.path.segments.clone();
            for seg in new_segments.iter_mut() {
                if let syn::PathArguments::AngleBracketed(args) = &mut seg.arguments {
                    for arg in args.args.iter_mut() {
                        if let syn::GenericArgument::Type(ty) = arg {
                            *ty = map_go_types(&*ty);
                        }
                    }
                }
            }
            syn::Type::Path(syn::TypePath {
                path: syn::Path { segments: new_segments, leading_colon: type_path.path.leading_colon.clone() },
                qself: type_path.qself.clone(),
            })
        }
        syn::Type::Reference(type_ref) => {
            let elem = map_go_types(&type_ref.elem);
            match &type_ref.lifetime {
                Some(l) => syn::Type::Reference(syn::TypeReference {
                    and_token: type_ref.and_token,
                    lifetime: Some(l.clone()),
                    mutability: type_ref.mutability,
                    elem: Box::new(elem),
                }),
                None => syn::Type::Reference(syn::TypeReference {
                    and_token: type_ref.and_token,
                    lifetime: None,
                    mutability: type_ref.mutability,
                    elem: Box::new(elem),
                }),
            }
        }
        syn::Type::Slice(type_array) => {
            let elem = map_go_types(&type_array.elem);
            syn::Type::Slice(syn::TypeSlice {
                bracket_token: type_array.bracket_token,
                elem: Box::new(elem),
            })
        }
        syn::Type::Array(a) => {
            let elem = map_go_types(&a.elem);
            syn::Type::Array(syn::TypeArray {
                bracket_token: a.bracket_token,
                semi_token: a.semi_token,
                len: a.len.clone(),
                elem: Box::new(elem),
            })
        }
        syn::Type::Tuple(type_tuple) => {
            let elems: Vec<_> = type_tuple.elems.iter().map(|t| map_go_types(t)).collect();
            let paren_token = type_tuple.paren_token;
            syn::Type::Tuple(syn::TypeTuple {
                paren_token,
                elems: elems.into_iter().collect(),
            })
        }
        syn::Type::Paren(inner) => {
            let mapped = map_go_types(&inner.elem);
            syn::Type::Paren(syn::TypeParen {
                paren_token: inner.paren_token,
                elem: Box::new(mapped),
            })
        }
        _ => ty.clone(),
    }
}
