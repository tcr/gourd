//! HIR Type representation.
//!
//! Provides a clean, strongly-typed representation of Go/Rust types
//! for use throughout the transpiler. This eliminates the need to store
//! type information as raw strings or token streams.

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
    Slice(Box<HirType>),      // `[]T`
    Array(Box<HirType>, usize), // `[T; N]`
    Map(Box<HirType>, Box<HirType>), // `map[K]V`
    Pointer(Box<HirType>),   // `*T` (Go pointer)
    Channel(Box<HirType>),   // `chan T`

    // Generic / trait types (used at runtime)
    GenericHirVec(Box<HirType>), // `Vec<T>`
    GenericHirHashMap, // `HashMap<K, V>` (keys/values stored separately)

    // Special types
    Error, // `error` → `Box<dyn std::error::Error>`
    Unit,  // `void` / no return type
    Unknown(String), // Fallback for unknown types

    // Rust reference types (used internally)
    Reference(Box<HirType>, Option<syn::Ident>), // `&T` or `&'a T`
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
            HirTypeKind::GenericHirVec(elem) => {
                let elem_ty = elem.to_rust_type();
                quote! { Vec<#elem_ty> }
            }
            HirTypeKind::GenericHirHashMap => quote! { ::gourd::prelude::HashMap },
            HirTypeKind::Error => quote! { Box<dyn std::error::Error> },
            HirTypeKind::Unit => TokenStream::new(),
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

    /// Check if this type is a slice type.
    pub fn is_slice(&self) -> bool {
        matches!(&self.kind, HirTypeKind::Slice(_))
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
}

impl HirTypeKind {
    /// Get a human-readable name for this type.
    pub fn name(&self) -> &str {
        match self {
            HirTypeKind::I32 => "i32",
            HirTypeKind::I64 => "i64",
            HirTypeKind::I8 => "i8",
            HirTypeKind::I16 => "i16",
            HirTypeKind::U8 => "u8",
            HirTypeKind::U16 => "u16",
            HirTypeKind::U32 => "u32",
            HirTypeKind::U64 => "u64",
            HirTypeKind::Usize => "usize",
            HirTypeKind::F32 => "f32",
            HirTypeKind::F64 => "f64",
            HirTypeKind::Bool => "bool",
            HirTypeKind::StringTy => "string",
            HirTypeKind::Char => "char",
            HirTypeKind::Slice(_) => "slice",
            HirTypeKind::Array(_, _) => "array",
            HirTypeKind::Map(_, _) => "map",
            HirTypeKind::Pointer(_) => "pointer",
            HirTypeKind::Channel(_) => "channel",
            HirTypeKind::GenericHirVec(_) => "Vec",
            HirTypeKind::GenericHirHashMap => "HashMap",
            HirTypeKind::Error => "error",
            HirTypeKind::Unit => "unit",
            HirTypeKind::Unknown(n) => n.as_str(),
            HirTypeKind::Reference(_, _) => "reference",
        }
    }
}

/// Map a Go type name string to an HIR type.
///
/// This is the canonical type mapping — the single source of truth for
/// how Go type names map to Rust types. Add new mappings here to support
/// new Go types.
pub fn go_type_to_hir(name: &str) -> HirType {
    match name {
        "int"    => HirType::new(HirTypeKind::I32),
        "int8"   => HirType::new(HirTypeKind::I8),
        "int16"  => HirType::new(HirTypeKind::I16),
        "int32"  => HirType::new(HirTypeKind::I32),
        "int64"  => HirType::new(HirTypeKind::I64),
        "uint"   => HirType::new(HirTypeKind::U32),
        "uint8"  => HirType::new(HirTypeKind::U8),
        "uint16" => HirType::new(HirTypeKind::U16),
        "uint32" => HirType::new(HirTypeKind::U32),
        "uint64" => HirType::new(HirTypeKind::U64),
        "uintptr" => HirType::new(HirTypeKind::Usize),
        "usize" => HirType::new(HirTypeKind::Usize),
        "byte"   => HirType::new(HirTypeKind::U8),
        "rune"   => HirType::new(HirTypeKind::Char),
        "float32" => HirType::new(HirTypeKind::F32),
        "float64" => HirType::new(HirTypeKind::F64),
        "string" => HirType::new(HirTypeKind::StringTy),
        "bool"   => HirType::new(HirTypeKind::Bool),
        "error"  => HirType::new(HirTypeKind::Error),
        _ => HirType::new(HirTypeKind::Unknown(name.to_string())),
    }
}

/// Map a Go type (possibly slice/map/pointer) to an HIR type.
///
/// Handles compound types like `[]int`, `map[string]int`, `*Foo`.
pub fn parse_go_type(input: &str) -> HirType {
    let input = input.trim();

    // Check for slice: `[]T`
    if input.starts_with("[]") {
        let elem = input.trim_start_matches("[]").trim();
        return HirType::new(HirTypeKind::Slice(Box::new(parse_go_type(elem))));
    }

    // Handle `__go_slice__` marker (empty slice type)
    if input == "__go_slice__" {
        // Unknown element type — use i32 as fallback
        return HirType::new(HirTypeKind::Slice(Box::new(go_type_to_hir("int"))));
    }

    // Check for map: `map[K]V`
    if let Some(map_start) = input.strip_prefix("map[") {
        if let Some(bracket_end) = map_start.find(']') {
            let key_str = &map_start[..bracket_end];
            let val_str = map_start[bracket_end + 1..].trim();
            let key = parse_go_type(key_str);
            let val = parse_go_type(val_str);
            return HirType::new(HirTypeKind::Map(Box::new(key), Box::new(val)));
        }
    }

    // Handle `__go_map[K, V]` marker from the old transpiler
    if let Some(rest) = input.strip_prefix("__go_map < ") {
        if let Some(bracket_end) = rest.rfind('>') {
            let inner = &rest[..bracket_end];
            // Split by `, ` to find key and value
            if let Some(comma_pos) = inner.find(", ") {
                let key_str = inner[..comma_pos].trim();
                let val_str = inner[comma_pos + 2..].trim();
                let key = parse_go_type(key_str);
                let val = parse_go_type(val_str);
                return HirType::new(HirTypeKind::Map(Box::new(key), Box::new(val)));
            }
        }
    }

    // Check for pointer: `*T`
    if let Some(elem) = input.strip_prefix('*') {
        return HirType::new(HirTypeKind::Pointer(Box::new(parse_go_type(elem.trim()))));
    }

    // Handle `__go_chan<T>` marker from the old transpiler
    if let Some(rest) = input.strip_prefix("__go_chan < ") {
        if let Some(bracket_end) = rest.rfind('>') {
            let elem_str = rest[..bracket_end].trim();
            let elem = parse_go_type(elem_str);
            return HirType::new(HirTypeKind::Channel(Box::new(elem)));
        }
    }

    // Primitive or unknown
    go_type_to_hir(input)
}
