//! HIR Type representation.
//!
//! Provides a clean, strongly-typed representation of Go/Rust types
//! for use throughout the transpiler. This eliminates the need to store
//! type information as raw strings or token streams.

use proc_macro2::{TokenStream, TokenTree, Delimiter};
use syn::Ident;
use crate::transpiler::hir::{HirStatement, HirBlock};
use crate::transpiler::hir::{HirExpr, HirExprKind};

/// A HIR type — the semantic representation of a Go or Rust type.
///
/// Unlike the current approach which stores types as `syn::Type` (parsing artifacts)
/// or `String` (lossy round-tripping), this captures the actual type structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirType {
    pub kind: HirTypeKind,
}

/// The possible kinds of HIR types.
///
/// This enum represents all types that can appear in Go code that we support.
/// Adding a new type only requires adding a variant here and updating the
/// type mapping in `hir/codegen.rs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirTypeKind {
    // Primitive types
    I32,
    I64,
    I8,
    I16,
    U8,
    U16,
    U32,
    U64,
    Usize,
    F32,
    F64,
    Bool,
    StringTy,
    Char,

    // Container types
    Slice(Box<HirType>),      // `[]T` → `Vec<T>` (owned)
    SliceRef(Box<HirType>),   // `&[]T` → `&[T]` (borrowed slice, for parameters)
    Array(Box<HirType>, usize), // `[T; N]`
    Map(Box<HirType>, Box<HirType>), // `map[K]V`
    Pointer(Box<HirType>),   // `*T` (Go pointer)
    Channel(Box<HirType>),   // `chan T`

    // Complex number types (Go complex64/complex128)
    Complex64,                // `complex64` → gourd::prelude::Complex64
    Complex128,               // `complex128` → gourd::prelude::Complex128

    // Generic / trait types (used at runtime)
    GenericHirVec(Box<HirType>), // `Vec<T>`
    GenericHirHashMap, // `HashMap<K, V>` (keys/values stored separately)

    // Special types
    Error, // `error` → `Box<dyn std::error::Error>`
    Unit,  // `void` / no return type
    Unknown(String), // Fallback for unknown types

    // Rust reference types (used internally)
    Reference(Box<HirType>, Option<syn::Ident>), // `&T` or `&'a T`

    // User-defined types (structs and interfaces)
    Struct { name: Ident, fields: Vec<(Ident, Box<HirType>)> },
    Interface { name: Ident, methods: Vec<HirInterfaceMethod> },
}

/// An HIR interface method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirInterfaceMethod {
    pub name: Ident,
    pub params: Vec<(Ident, Box<HirType>)>,
    pub returns: Vec<Box<HirType>>,
}

/// HIR representation of a receiver function (impl block method).
// Body is stored as raw tokens to be transpiled via go_to_rust later.
pub struct HirReceiverFn {
    pub recv_name: Ident,
    pub recv_type: HirType,
    pub pointer: bool,
    pub fn_name: Ident,
    pub params: Vec<(Ident, Box<HirType>)>,
    pub returns: Vec<Box<HirType>>,
    pub body: Option<proc_macro2::TokenStream>,
}

impl HirType {
    /// Create a new HIR type.
    pub fn new(kind: HirTypeKind) -> Self {
        HirType { kind }
    }

    /// Get the Rust type string for code generation.
    pub fn to_rust_type(&self) -> proc_macro2::TokenStream {
        use proc_macro2::TokenStream;
        use quote::quote;

        match &self.kind {
            HirTypeKind::I32 => quote! { i32 },
            HirTypeKind::I64 => quote! { i64 },
            HirTypeKind::I8 => quote! { i8 },
            HirTypeKind::I16 => quote! { i16 },
            HirTypeKind::U8 => quote! { u8 },
            HirTypeKind::U16 => quote! { u16 },
            HirTypeKind::U32 => quote! { u32 },
            HirTypeKind::U64 => quote! { u64 },
            HirTypeKind::Usize => quote! { usize },
            HirTypeKind::F32 => quote! { f32 },
            HirTypeKind::F64 => quote! { f64 },
            HirTypeKind::Bool => quote! { bool },
            HirTypeKind::StringTy => quote! { String },
            HirTypeKind::Char => quote! { char },
            HirTypeKind::Slice(elem) => {
                // Owned slice in expression/variable position → Vec<T>
                let elem_ty = elem.to_rust_type();
                quote! { Vec<#elem_ty> }
            }
            HirTypeKind::SliceRef(elem) => {
                // Borrowed slice in parameter position → &[T]
                let elem_ty = elem.to_rust_type();
                quote! { &[#elem_ty] }
            }
            HirTypeKind::Array(elem, n) => {
                let elem_ty = elem.to_rust_type();
                let n = *n as u32;
                quote! { [&#elem_ty; #n] }
            }
            HirTypeKind::Map(key, val) => {
                let key_ty = key.to_rust_type();
                let val_ty = val.to_rust_type();
                quote! { ::gourd::prelude::HashMap<#key_ty, #val_ty> }
            }
            HirTypeKind::Pointer(elem) => {
                let elem_ty = elem.to_rust_type();
                quote! { *mut #elem_ty }
            }
            HirTypeKind::Channel(elem) => {
                let elem_ty = elem.to_rust_type();
                quote! { GoChannel<#elem_ty> }
            }
            HirTypeKind::Complex64 => quote! { ::gourd::prelude::Complex64 },
            HirTypeKind::Complex128 => quote! { ::gourd::prelude::Complex128 },
            HirTypeKind::GenericHirVec(elem) => {
                let elem_ty = elem.to_rust_type();
                quote! { Vec<#elem_ty> }
            }
            HirTypeKind::GenericHirHashMap => quote! { ::gourd::prelude::HashMap },
            HirTypeKind::Error => quote! { Box<dyn std::error::Error> },
            HirTypeKind::Unit => TokenStream::new(),
            HirTypeKind::Struct { name, fields } => {
                let field_tokens: Vec<_> = fields.iter().map(|(field_name, field_type)| {
                    let ty = field_type.to_rust_type();
                    quote! { pub #field_name: #ty }
                }).collect();
                quote! { struct #name { #(#field_tokens),* } }
            }
            HirTypeKind::Interface { name, methods } => {
                let method_tokens: Vec<_> = methods.iter().map(|method| {
                    let param_tokens: Vec<_> = method.params.iter().map(|(param_name, param_type)| {
                        let ty = param_type.to_rust_type();
                        quote! { #param_name: #ty }
                    }).collect();
                    let return_tokens = if method.returns.is_empty() {
                        quote! {}
                    } else if method.returns.len() == 1 {
                        let ty = method.returns[0].to_rust_type();
                        quote! { -> #ty }
                    } else {
                        let return_types: Vec<_> = method.returns.iter().map(|t| t.to_rust_type()).collect();
                        quote! { -> ( #(#return_types),* ) }
                    };
                    let method_name = &method.name;
                    quote! { fn #method_name #(#param_tokens),* #return_tokens }
                }).collect();
                quote! { trait #name { #(#method_tokens)* } }
            }
            HirTypeKind::Unknown(name) => {
                // Emit as identifier (not string literal).
                // For known Rust types like usize, u8, etc., this works directly.
                // For truly unknown types, the compiler will give a proper error.
                let id: syn::Ident = syn::parse_str(&name).unwrap_or_else(|_| syn::Ident::new(&name, proc_macro2::Span::call_site()));
                quote! { #id }
            }
            HirTypeKind::Reference(elem, lifetime) => {
                let elem_ty = elem.to_rust_type();
                if let Some(lt) = lifetime {
                    quote! { &#lt #elem_ty }
                } else {
                    quote! { &#elem_ty }
                }
            }
        }
    }

    /// Return Go-style wrapper types for interface method signatures.
    ///
    /// In Go interfaces, strings and slices use wrapper types (GoString, GoSlice)
    /// that provide proper Go semantics (reference-copy rather than move).
    pub fn to_interface_type(&self) -> proc_macro2::TokenStream {
        use proc_macro2::TokenStream;
        use quote::quote;

        match &self.kind {
            HirTypeKind::StringTy => quote! { GoString },
            HirTypeKind::Slice(_) | HirTypeKind::SliceRef(_) => {
                // Slices in interfaces are GoSlice<u8>
                quote! { GoSlice<u8> }
            }
            // All other types use the standard Rust mapping
            _ => self.to_rust_type(),
        }
    }

    /// Check if this type is a primitive (scalar) type.
    pub fn is_primitive(&self) -> bool {
        matches!(&self.kind,
            HirTypeKind::I32 | HirTypeKind::I64 | HirTypeKind::I8 | HirTypeKind::I16
            | HirTypeKind::U8 | HirTypeKind::U16 | HirTypeKind::U32 | HirTypeKind::U64
            | HirTypeKind::Usize | HirTypeKind::F32 | HirTypeKind::F64
            | HirTypeKind::Bool | HirTypeKind::Char
        )
    }

    /// Check if this type is numeric.
    pub fn is_numeric(&self) -> bool {
        matches!(&self.kind,
            HirTypeKind::I32 | HirTypeKind::I64 | HirTypeKind::I8 | HirTypeKind::I16
            | HirTypeKind::U8 | HirTypeKind::U16 | HirTypeKind::U32 | HirTypeKind::U64
            | HirTypeKind::Usize | HirTypeKind::F32 | HirTypeKind::F64
        )
    }

    /// Check if this type is a string type.
    pub fn is_string(&self) -> bool {
        matches!(&self.kind, HirTypeKind::StringTy)
    }

    /// Check if this type is a slice type (owned or borrowed).
    pub fn is_slice(&self) -> bool {
        matches!(&self.kind, HirTypeKind::Slice(_) | HirTypeKind::SliceRef(_))
    }

    /// Check if this type is a map type.
    pub fn is_map(&self) -> bool {
        matches!(&self.kind, HirTypeKind::Map(_, _))
    }

    /// Get the element type if this is a slice, map, channel, or pointer.
    pub fn element_type(&self) -> Option<&HirType> {
        match &self.kind {
            HirTypeKind::Slice(elem)
            | HirTypeKind::Pointer(elem)
            | HirTypeKind::Channel(elem) => Some(elem),
            HirTypeKind::Map(key, val) => {
                // Returns value type for maps
                Some(val)
            }
            HirTypeKind::GenericHirVec(elem) => Some(elem),
            _ => None,
        }
    }

    /// Get the key type if this is a map.
    pub fn key_type(&self) -> Option<&HirType> {
        if let HirTypeKind::Map(key, _) = &self.kind {
            Some(key)
        } else {
            None
        }
    }

    /// Check if this type is a struct.
    pub fn is_struct(&self) -> bool {
        matches!(&self.kind, HirTypeKind::Struct { .. })
    }

    /// Check if this type is an interface.
    pub fn is_interface(&self) -> bool {
        matches!(&self.kind, HirTypeKind::Interface { .. })
    }
}

impl HirTypeKind {
    /// Get a human-readable name for this type.
    pub fn name(&self) -> String {
        match self {
            HirTypeKind::I32 => "i32".to_string(),
            HirTypeKind::I64 => "i64".to_string(),
            HirTypeKind::I8 => "i8".to_string(),
            HirTypeKind::I16 => "i16".to_string(),
            HirTypeKind::U8 => "u8".to_string(),
            HirTypeKind::U16 => "u16".to_string(),
            HirTypeKind::U32 => "u32".to_string(),
            HirTypeKind::U64 => "u64".to_string(),
            HirTypeKind::Usize => "usize".to_string(),
            HirTypeKind::F32 => "f32".to_string(),
            HirTypeKind::F64 => "f64".to_string(),
            HirTypeKind::Bool => "bool".to_string(),
            HirTypeKind::StringTy => "string".to_string(),
            HirTypeKind::Char => "char".to_string(),
            HirTypeKind::Slice(_) | HirTypeKind::SliceRef(_) => "slice".to_string(),
            HirTypeKind::Array(_, _) => "array".to_string(),
            HirTypeKind::Map(_, _) => "map".to_string(),
            HirTypeKind::Pointer(_) => "pointer".to_string(),
            HirTypeKind::Channel(_) => "channel".to_string(),
            HirTypeKind::GenericHirVec(_) => "Vec".to_string(),
            HirTypeKind::GenericHirHashMap => "HashMap".to_string(),
            HirTypeKind::Complex64 => "complex64".to_string(),
            HirTypeKind::Complex128 => "complex128".to_string(),
            HirTypeKind::Error => "error".to_string(),
            HirTypeKind::Unit => "unit".to_string(),
            HirTypeKind::Unknown(n) => n.clone(),
            HirTypeKind::Reference(_, _) => "reference".to_string(),
            HirTypeKind::Struct { name, .. } => name.to_string(),
            HirTypeKind::Interface { name, .. } => name.to_string(),
        }
    }
}
