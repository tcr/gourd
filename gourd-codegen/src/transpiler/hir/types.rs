//! HIR Type representation.
//!
//! Provides a clean, strongly-typed representation of Go/Rust types
//! for use throughout the transpiler. This eliminates the need to store
//! type information as raw strings or token streams.

use syn::Ident;
use proc_macro2::{TokenStream, TokenTree, Delimiter};
use quote::quote;
use super::statement::{HirStatement, HirBlock};
use super::expression::{HirExpr, HirExprKind};

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
            HirTypeKind::StringTy => quote! { ::gourd::GoString },
            HirTypeKind::Char => quote! { char },
            HirTypeKind::Slice(elem) => {
                // Owned slice in expression/variable position → GoSlice<T>
                let elem_ty = elem.to_rust_type();
                quote! { ::gourd::GoSlice<#elem_ty> }
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
            HirTypeKind::GenericHirVec(elem) => {
                // Generic vec (e.g. from slice literals) → GoSlice<T>
                let elem_ty = elem.to_rust_type();
                quote! { ::gourd::GoSlice<#elem_ty> }
            }
            HirTypeKind::GenericHirHashMap => quote! { ::gourd::GoMap },
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
            HirTypeKind::StringTy => "GoString".to_string(),
            HirTypeKind::Char => "char".to_string(),
            HirTypeKind::Slice(_) | HirTypeKind::SliceRef(_) => "slice".to_string(),
            HirTypeKind::Array(_, _) => "array".to_string(),
            HirTypeKind::Map(_, _) => "map".to_string(),
            HirTypeKind::Pointer(_) => "pointer".to_string(),
            HirTypeKind::Channel(_) => "channel".to_string(),
            HirTypeKind::GenericHirVec(_) => "Vec".to_string(),
            HirTypeKind::GenericHirHashMap => "GoMap".to_string(),
            HirTypeKind::Error => "error".to_string(),
            HirTypeKind::Unit => "unit".to_string(),
            HirTypeKind::Unknown(n) => n.clone(),
            HirTypeKind::Reference(_, _) => "reference".to_string(),
            HirTypeKind::Struct { name, .. } => name.to_string(),
            HirTypeKind::Interface { name, .. } => name.to_string(),
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
    parse_go_type_inner(input, 0)
}

fn parse_go_type_inner(input: &str, depth: u32) -> HirType {
    const MAX_DEPTH: u32 = 32;
    let input = input.trim();
    if input.is_empty() {
        return HirType::new(HirTypeKind::Unknown("empty_type".to_string()));
    }
    if depth > MAX_DEPTH {
        return HirType::new(HirTypeKind::Unknown(input.to_string()));
    }

    // Check for slice: `[]T`
    if input.starts_with("[]") {
        let elem = input.trim_start_matches("[]").trim();
        return HirType::new(HirTypeKind::Slice(Box::new(parse_go_type_inner(elem, depth + 1))));
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
            let key = parse_go_type_inner(key_str, depth + 1);
            let val = parse_go_type_inner(val_str, depth + 1);
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
                let key = parse_go_type_inner(key_str, depth + 1);
                let val = parse_go_type_inner(val_str, depth + 1);
                return HirType::new(HirTypeKind::Map(Box::new(key), Box::new(val)));
            }
        }
    }

    // Check for pointer: `*T`
    if let Some(elem) = input.strip_prefix('*') {
        return HirType::new(HirTypeKind::Pointer(Box::new(parse_go_type_inner(elem.trim(), depth + 1))));
    }

    // Handle `__go_chan<T>` marker from the old transpiler
    if let Some(rest) = input.strip_prefix("__go_chan < ") {
        if let Some(bracket_end) = rest.rfind('>') {
            let elem_str = rest[..bracket_end].trim();
            let elem = parse_go_type_inner(elem_str, depth + 1);
            return HirType::new(HirTypeKind::Channel(Box::new(elem)));
        }
    }

    // Handle `chan < T >` format (from quote output, with spaces)
    if let Some(rest) = input.strip_prefix("chan < ") {
        if let Some(bracket_end) = rest.rfind('>') {
            let elem_str = rest[..bracket_end].trim();
            let elem = parse_go_type_inner(elem_str, depth + 1);
            return HirType::new(HirTypeKind::Channel(Box::new(elem)));
        }
    }

    // Handle `GoChannel < T >` format (from map_go_types output)
    if let Some(rest) = input.strip_prefix("GoChannel < ") {
        if let Some(bracket_end) = rest.rfind('>') {
            let elem_str = rest[..bracket_end].trim();
            let elem = parse_go_type_inner(elem_str, depth + 1);
            return HirType::new(HirTypeKind::Channel(Box::new(elem)));
        }
    }

    // Handle `gourd :: prelude :: HashMap < K, V >` format (from map_go_types output)
    if let Some(rest) = input.strip_prefix("gourd :: prelude :: HashMap < ") {
        if let Some(bracket_end) = rest.rfind('>') {
            let inner_str = rest[..bracket_end].trim();
            // Split by `, ` to get key and value
            if let Some(comma_pos) = inner_str.find(", ") {
                let key_str = inner_str[..comma_pos].trim();
                let val_str = inner_str[comma_pos + 2..].trim();
                let key = parse_go_type_inner(key_str, depth + 1);
                let val = parse_go_type_inner(val_str, depth + 1);
                return HirType::new(HirTypeKind::Map(Box::new(key), Box::new(val)));
            }
        }
    }

    // Handle `Vec < T >` format (from map_go_types output on slice types)
    if let Some(rest) = input.strip_prefix("Vec < ") {
        if let Some(bracket_end) = rest.rfind('>') {
            let elem_str = rest[..bracket_end].trim();
            let elem = parse_go_type_inner(elem_str, depth + 1);
            return HirType::new(HirTypeKind::Slice(Box::new(elem)));
        }
    }

    // Primitive or unknown
    go_type_to_hir(input)
}

/// Parse a Go struct declaration directly into a HIR struct type.
///
/// This bypasses the Go AST (`GoStruct`) entirely and produces HIR types.
pub fn parse_go_struct(input: TokenStream) -> Option<HirType> {
    use proc_macro2::TokenStream;
    use quote::ToTokens;

    let trees: Vec<TokenTree> = input.into_iter().collect();
    if trees.is_empty() {
        return None;
    }

    // Must start with `struct` keyword
    let first = &trees[0];
    let struct_name_idx = match first {
        TokenTree::Ident(id) => {
            let name_str = id.to_string();
            if name_str == "struct" || name_str == "type" {
                if trees.len() > 1 { 1 } else { return None; }
            } else {
                return None;
            }
        }
        _ => return None,
    };

    // Get struct name
    let struct_name = match &trees[struct_name_idx] {
        TokenTree::Ident(id) => id.clone(),
        _ => return None,
    };

    // Find the struct body — looks for `{ ... }`
    let mut body_start = None;
    for (i, tt) in trees.iter().enumerate() {
        if let TokenTree::Group(g) = tt {
            if g.delimiter() == Delimiter::Brace {
                body_start = Some(i);
                break;
            }
        }
    }

    let body_start = match body_start {
        Some(idx) => idx,
        None => return None,
    };

    // Parse fields from the brace group
    let body_group = match &trees[body_start] {
        TokenTree::Group(g) => g,
        _ => return None,
    };

    let mut fields: Vec<(Ident, Box<HirType>)> = Vec::new();
    let body_trees: Vec<TokenTree> = body_group.stream().into_iter().collect();

    // Parse fields: `name type` pairs, separated by commas/semicolons
    let mut i = 0;
    while i < body_trees.len() {
        // Skip any punctuations
        if let TokenTree::Punct(p) = &body_trees[i] {
            if p.as_char() == ',' || p.as_char() == ';' || p.as_char() == '{' || p.as_char() == '}' {
                i += 1;
                continue;
            }
        }

        // Try to parse `name type` pair
        if let TokenTree::Ident(name_id) = &body_trees[i] {
            let field_name = name_id.clone();

            // Skip if this is a reserved keyword
            let name_str = name_id.to_string();
            if matches!(name_str.as_str(),
                "bool" | "string" | "int" | "int8" | "int16" | "int32" | "int64"
                | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr"
                | "byte" | "rune" | "float32" | "float64" | "error"
                | "if" | "else" | "for" | "return" | "switch" | "case" | "default"
                | "type" | "struct" | "func" | "interface" | "package" | "import"
                | "const" | "var" | "chan" | "map" | "*" | "[]" | "<-")
            {
                i += 1;
                continue;
            }

            // Find the type — must be next non-punctuation token
            if i + 1 < body_trees.len() {
                let type_idx = i + 1;
                if let TokenTree::Ident(type_id) = &body_trees[type_idx] {
                    let type_str = type_id.to_string();
                    // Skip type if it's a keyword or special token
                    if !matches!(type_str.as_str(),
                        "bool" | "string" | "int" | "int8" | "int16" | "int32" | "int64"
                        | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr"
                        | "byte" | "rune" | "float32" | "float64" | "error")
                    {
                        // It's a custom type — try to parse as HIR type
                        // For now, treat it as unknown
                        let ty = HirType::new(HirTypeKind::Unknown(type_str.clone()));
                        fields.push((field_name, Box::new(ty)));
                        i += 2;
                        continue;
                    }
                }

                // Type might be a primitive — parse it
                let type_token = &body_trees[type_idx];
                if let TokenTree::Ident(type_id) = type_token {
                    let type_str = type_id.to_string();
                    let ty = go_type_to_hir(&type_str);
                    fields.push((field_name, Box::new(ty)));
                    i += 2;
                    continue;
                }
            }
        }
        i += 1;
    }

    if fields.is_empty() {
        return None;
    }

    Some(HirType::new(HirTypeKind::Struct {
        name: struct_name,
        fields,
    }))
}

/// Parse a Go interface declaration directly into a HIR interface type.
///
/// This bypasses the Go AST (`GoInterface`) entirely and produces HIR types.
pub fn parse_go_interface(input: TokenStream) -> Option<HirType> {
    use proc_macro2::TokenStream;
    use quote::ToTokens;

    let trees: Vec<TokenTree> = input.into_iter().collect();
    if trees.is_empty() {
        return None;
    }

    // Must start with `interface` keyword
    let first = &trees[0];
    if let TokenTree::Ident(id) = first {
        let name_str = id.to_string();
        if name_str != "interface" {
            return None;
        }
    } else {
        return None;
    }

    // Skip the interface keyword and get the interface name
    let interface_name_idx = if trees.len() > 1 {
        1
    } else {
        return None;
    };

    let interface_name = match &trees[interface_name_idx] {
        TokenTree::Ident(id) => id.clone(),
        _ => return None,
    };


    // Find the interface body — looks for `{ ... }`
    let mut body_start = None;
    for (i, tt) in trees.iter().enumerate() {
        if let TokenTree::Group(g) = tt {
            if g.delimiter() == Delimiter::Brace {
                body_start = Some(i);
                break;
            }
        }
    }

    let body_start = match body_start {
        Some(idx) => idx,
        None => return None,
    };

    let body_group = match &trees[body_start] {
        TokenTree::Group(g) => g,
        _ => return None,
    };

    let mut methods: Vec<HirInterfaceMethod> = Vec::new();
    let body_trees: Vec<TokenTree> = body_group.stream().into_iter().collect();

    // Parse methods: `name(input types) output types` patterns
    let mut i = 0;
    let mut loop_count = 0;
    while i < body_trees.len() {
        loop_count += 1;
        if loop_count > 1000 {
            break;
        }
        if let TokenTree::Ident(method_id) = &body_trees[i] {
            let method_name_str = method_id.to_string();

            // Skip reserved keywords and type names
            if matches!(method_name_str.as_str(),
                "bool" | "string" | "int" | "int8" | "int16" | "int32" | "int64"
                | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr"
                | "byte" | "rune" | "float32" | "float64" | "error"
                | "if" | "else" | "for" | "return" | "switch" | "case" | "default"
                | "type" | "struct" | "func" | "interface" | "package" | "import"
                | "const" | "var" | "chan" | "map" | "*" | "[]" | "<-")
            {
                i += 1;
                continue;
            }

            // Look for `(` after method name
            if i + 1 < body_trees.len() {
                if let TokenTree::Group(params_group) = &body_trees[i + 1] {
                    if params_group.delimiter() == Delimiter::Parenthesis {
                        // Parse parameters
                        let params: Vec<(Ident, Box<HirType>)> = parse_interface_params(params_group.stream());

                        // Find return type after closing paren
                        let mut returns: Vec<Box<HirType>> = Vec::new();
                        let mut next_i = i + 1;
                        if i + 2 < body_trees.len() {
                            let return_idx = i + 2;
                            next_i = return_idx + 1;
                            // Check for return type
                            if let TokenTree::Ident(ret_id) = &body_trees[return_idx] {
                                let ret_str = ret_id.to_string();
                                let ty = parse_go_type(&ret_str);
                                returns.push(Box::new(ty));
                            } else if let TokenTree::Group(ret_group) = &body_trees[return_idx] {
                                // Handle slice return types like []byte
                                if ret_group.delimiter() == Delimiter::Bracket {
                                    let ret_content: Vec<TokenTree> = ret_group.stream().into_iter().collect();
                                    if ret_content.is_empty() {
                                        // Empty bracket group — element type is the next token
                                        if return_idx + 1 < body_trees.len() {
                                            if let TokenTree::Ident(elem_id) = &body_trees[return_idx + 1] {
                                                let elem_str = elem_id.to_string();
                                                let elem_ty = go_type_to_hir(&elem_str);
                                                // Return types use Vec (owned), params use SliceRef (&[T])
                                                returns.push(Box::new(HirType::new(HirTypeKind::Slice(Box::new(elem_ty)))));
                                                next_i = return_idx + 2;
                                            }
                                        }
                                    } else if let TokenTree::Ident(elem_id) = &ret_content[0] {
                                        // Bracket contains element type directly (e.g. []int)
                                        let elem_str = elem_id.to_string();
                                        let elem_ty = go_type_to_hir(&elem_str);
                                        // Return types use Vec (owned), params use SliceRef (&[T])
                                        returns.push(Box::new(HirType::new(HirTypeKind::Slice(Box::new(elem_ty)))));
                                    }
                                }
                            }
                        }

                        methods.push(HirInterfaceMethod {
                            name: method_id.clone(),
                            params,
                            returns,
                        });

                        // Skip past method definition
                        i = next_i;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }

    // Empty interfaces are valid
    let result = Some(HirType::new(HirTypeKind::Interface {
        name: interface_name,
        methods,
    }));
    let interface_name_str = result.as_ref().unwrap().kind.name().to_string();
    let methods_len = match &result.as_ref().unwrap().kind {
        HirTypeKind::Interface { methods, .. } => methods.len(),
        _ => 0,
    };
    result
}

/// Parse interface parameters from a parenthesis group.
/// Handles simple types, slice types (e.g., `[]byte`), and grouped parameters (e.g., `a, b int`).
fn parse_interface_params(input: TokenStream) -> Vec<(Ident, Box<HirType>)> {
    let trees: Vec<TokenTree> = input.into_iter().collect();
    let mut params: Vec<(Ident, Box<HirType>)> = Vec::new();
    let mut i = 0;
    let max_iterations = trees.len() + 100; // Prevent infinite loops
    let mut named_params: Vec<Ident> = Vec::new(); // Collect names before a grouped type

    while i < trees.len() && max_iterations > 0 {
        // Try to parse `name type` pair or named parameter
        if let TokenTree::Ident(name_id) = &trees[i] {
            let name_str = name_id.to_string();
            // Skip Go keywords that aren't parameter names
            if matches!(name_str.as_str(), "func" | "type" | "struct" | "interface") {
                i += 1;
                continue;
            }

            // Collect this name as a potential parameter
            named_params.push(name_id.clone());
            i += 1;

            // Check if next token is a type (not a comma or punct)
            if i < trees.len() {
                // Slice type: []T
                if let TokenTree::Group(slice_group) = &trees[i] {
                    if slice_group.delimiter() == proc_macro2::Delimiter::Bracket {
                        // Parse the slice element type
                        // []T is parsed as [] (empty bracket group) followed by T
                        let slice_content: Vec<TokenTree> = slice_group.stream().into_iter().collect();
                        let elem_str = if slice_content.is_empty() {
                            // Empty bracket group — element type is the next token
                            if i + 1 < trees.len() {
                                if let TokenTree::Ident(elem_id) = &trees[i + 1] {
                                    elem_id.to_string()
                                } else {
                                    "int".to_string()
                                }
                            } else {
                                "int".to_string()
                            }
                        } else if let TokenTree::Ident(elem_id) = &slice_content[0] {
                            elem_id.to_string()
                        } else {
                            "int".to_string()
                        };
                        let elem_ty = go_type_to_hir(&elem_str);
                        // Skip the next token if we used it (element type after empty bracket)
                        if slice_content.is_empty() && i + 1 < trees.len() {
                            i += 1;
                        }

                        // If we have named params waiting, assign them all to this slice type
                        for n in named_params.drain(..) {
                            params.push((n, Box::new(HirType::new(HirTypeKind::SliceRef(Box::new(elem_ty.clone()))))));
                        }
                        i += 1;
                        continue;
                    }
                }

                // Simple type
                if let TokenTree::Ident(type_id) = &trees[i] {
                    let type_str = type_id.to_string();
                    // If we have a single named param, assign it
                    if named_params.len() == 1 {
                        let name = named_params.pop().unwrap();
                        let ty = go_type_to_hir(&type_str);
                        params.push((name, Box::new(ty)));
                    } else if !named_params.is_empty() {
                        // Multiple names with single type = grouped params
                        for n in named_params.drain(..) {
                            let ty = go_type_to_hir(&type_str);
                            params.push((n, Box::new(ty)));
                        }
                    }
                    i += 1;
                    continue;
                }

                // Punctuation (comma) - keep collecting names for grouped params
                if let TokenTree::Punct(p) = &trees[i] {
                    if p.as_char() == ',' {
                        i += 1;
                        continue;
                    }
                }
            }

            // If we have named params but no type follows, skip
            named_params.clear();
        } else {
            // Non-ident token - skip
            i += 1;
        }
    }

    params
}

/// Parse a Go receiver function directly into HIR.
///
/// Input: `func (recv Type) name(params) output { body }`
/// Parsed into: HirReceiverFn with receiver, params, returns, and body.
pub fn parse_go_receiver_fn(input: TokenStream) -> Option<HirReceiverFn> {
    let trees: Vec<TokenTree> = input.into_iter().collect();
    if trees.len() < 4 {
        return None;
    }

    // tree[0] = "func"
    // tree[1] = receiver group (Parenthesis)
    // tree[2] = function name (Ident)
    // tree[3+] = parameters (Parenthesis), optional output, body (Brace)

    // Parse receiver from tree[1]
    let recv_group = match &trees[1] {
        TokenTree::Group(g) if g.delimiter() == Delimiter::Parenthesis => g.stream(),
        _ => return None,
    };
    let recv_trees: Vec<TokenTree> = recv_group.into_iter().collect();
    if recv_trees.len() < 2 {
        return None;
    }

    // Parse receiver name
    let recv_name = match &recv_trees[0] {
        TokenTree::Ident(id) => id.clone(),
        _ => return None,
    };

    // Parse receiver type (may have * prefix for pointer receivers)
    let (pointer, recv_type) = match &recv_trees[1] {
        TokenTree::Punct(p) if p.as_char() == '*' => {
            // Pointer receiver: skip the *
            if recv_trees.len() < 3 {
                return None;
            }
            match &recv_trees[2] {
                TokenTree::Ident(type_id) => {
                    (true, go_type_to_hir(&type_id.to_string()))
                }
                _ => return None,
            }
        }
        TokenTree::Ident(type_id) => {
            // Value receiver
            (false, go_type_to_hir(&type_id.to_string()))
        }
        _ => return None,
    };

    // Parse function name
    let fn_name = match &trees[2] {
        TokenTree::Ident(id) => id.clone(),
        _ => return None,
    };

    // Parse parameters from tree[3] if it's a paren group
    let mut params: Vec<(Ident, Box<HirType>)> = Vec::new();
    let mut remaining = if trees.len() > 3 {
        if let TokenTree::Group(g) = &trees[3] {
            if g.delimiter() == Delimiter::Parenthesis {
                params = parse_interface_params(g.stream());
            }
        }
        if trees.len() > 4 { 4 } else { return Some(HirReceiverFn {
            recv_name, recv_type, pointer, fn_name, params,
            returns: Vec::new(), body: None,
        }); }
    } else { return Some(HirReceiverFn {
        recv_name, recv_type, pointer, fn_name, params,
        returns: Vec::new(), body: None,
    }); };

    // Parse optional return type(s)
    let mut returns: Vec<Box<HirType>> = Vec::new();
    if remaining < trees.len() {
        if let TokenTree::Ident(ret_id) = &trees[remaining] {
            let ret_str = ret_id.to_string();
            returns.push(Box::new(parse_go_type(&ret_str)));
            remaining += 1;
        }
    }

    // Parse body from tree[remaining] if it's a brace group
    if remaining < trees.len() {
        if let TokenTree::Group(g) = &trees[remaining] {
            if g.delimiter() == Delimiter::Brace {
                let body_tokens: proc_macro2::TokenStream = g.stream();
                return Some(HirReceiverFn {
                    recv_name, recv_type, pointer, fn_name, params,
                    returns, body: Some(body_tokens),
                });
            }
        }
    }

    // No body found — return with what we have
    Some(HirReceiverFn {
        recv_name, recv_type, pointer, fn_name, params,
        returns, body: None,
    })
}

// ─── HIR types for select and switch statements ──────────────────────────────

/// HIR representation of a Go select statement.
pub struct HirSelect {
    pub cases: Vec<HirSelectCase>,
    pub default_body: Option<HirBlock>,
}

/// A single case in a Go select statement.
pub enum HirSelectCase {
    /// `case ch <- value:` — send case
    Send {
        ch: Box<HirExpr>,
        value: Box<HirExpr>,
    },
    /// `case <-ch:` — receive case  
    Recv {
        ch: Box<HirExpr>,
    },
    /// `default:` — default case
    Default,
}

/// HIR representation of a Go switch statement.
pub struct HirSwitch {
    pub selector: Option<Box<HirExpr>>,
    pub cases: Vec<HirSwitchCase>,
    pub default_body: Option<HirBlock>,
}

/// A single case in a Go switch statement.
pub struct HirSwitchCase {
    pub patterns: Vec<HirExpr>,
    pub body: HirBlock,
}


#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn test_parse_interface_basic() {
        let input = quote! { interface Shape { Name() string } };
        let result = parse_go_interface(input);
        assert!(result.is_some(), "Expected some result for basic interface");
    }

    #[test]
    fn test_parse_interface_empty() {
        let input = quote! { interface Empty {} };
        let result = parse_go_interface(input);
        // Empty interfaces are valid — should return Some with no methods
        assert!(result.is_some(), "Expected Some for empty interface");
        let ty = result.unwrap();
        match &ty.kind {
            HirTypeKind::Interface { name, methods } => {
                assert_eq!(name.to_string(), "Empty");
                assert!(methods.is_empty());
            }
            _ => panic!("Expected Interface kind"),
        }
    }

    #[test]
    fn test_parse_receiver_fn_value() {
        let input = quote! { func (s Foo) GetName() string { s.name } };
        let result = parse_go_receiver_fn(input);
        assert!(result.is_some(), "Expected Some for value receiver");
        let rf = result.unwrap();
        assert_eq!(rf.fn_name.to_string(), "GetName");
        assert!(!rf.pointer);
    }

    #[test]
    fn test_parse_receiver_fn_pointer() {
        let input = quote! { func (s *Foo) SetName(n string) { s.name = n } };
        let result = parse_go_receiver_fn(input);
        assert!(result.is_some(), "Expected Some for pointer receiver");
        let rf = result.unwrap();
        assert_eq!(rf.fn_name.to_string(), "SetName");
        assert!(rf.pointer);
    }
}

// ============================================================
// Legacy type mapping functions — moved from transpiler/types.rs
// These support parsing Go types during conversion.
// ============================================================

use syn::Token;

/// Map a Go type string (e.g., "int", "string", "rune") to a Rust type string.
/// Used for parsing `make()` call arguments where syn can't parse Go types.
pub(crate) fn map_go_type_str(go_type: &str) -> syn::Type {
    let rust_type = match go_type.trim() {
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
        "string" => "::gourd::GoString",
        "bool" => "bool",
        "error" => "Box<dyn std::error::Error>",
        _ => "unknown",
    };
    syn::parse_str::<syn::Type>(rust_type).unwrap_or_else(|_| {
        // Fallback: construct a Type::Path manually.
        // If the type string contains '::', it's a qualified path like "::gourd::GoString".
        let (is_global, type_str) = if rust_type.starts_with("::") {
            (true, &rust_type[2..])
        } else {
            (false, rust_type)
        };
        let segments: Vec<syn::Ident> = type_str
            .split("::")
            .filter(|s| !s.is_empty())
            .map(|s| syn::Ident::new(s, proc_macro2::Span::call_site()))
            .collect();
        let path = if segments.is_empty() {
            syn::Path::from(syn::Ident::new("unknown", proc_macro2::Span::call_site()))
        } else if is_global {
            let mut path_segments = syn::punctuated::Punctuated::new();
            for seg in segments {
                path_segments.push(syn::PathSegment::from(seg));
            }
            syn::Path {
                leading_colon: Some(syn::Token![::](proc_macro2::Span::call_site())),
                segments: path_segments,
            }
        } else {
            syn::Path::from(segments.first().cloned().unwrap_or_else(|| syn::Ident::new("unknown", proc_macro2::Span::call_site())))
        };
        syn::Type::Path(syn::TypePath { path, qself: None })
    })
}

/// Map a single Go type identifier to its Rust equivalent.
/// Returns a `syn::Type` so that generic parameters can be recursed into.
pub(crate) fn map_go_types(ty: &syn::Type) -> syn::Type {
    match ty {
        syn::Type::Path(type_path) => {
            // Check for `__go_chan<T>` marker - converted to `GoChannel::<T>`
            if type_path.path.segments.len() == 1 {
                let first_name = type_path.path.segments.first().unwrap().ident.to_string();
                // Check for Go `chan T` syntax
                if first_name == "chan" {
                    // Extract element type from generic args: `chan<T>`
                    let seg = &type_path.path.segments[0];
                    if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                        if let Some(syn::GenericArgument::Type(elem_ty)) = args.args.first() {
                            // Map the element type first
                            let mapped_elem = map_go_types(elem_ty);
                            // Build GoChannel<T> with the mapped element type
                            let mut chan_path = syn::Path::from(syn::Ident::new("GoChannel", proc_macro2::Span::call_site()));
                            chan_path.segments.clear();
                            chan_path.segments.push(syn::PathSegment {
                                ident: syn::Ident::new("GoChannel", proc_macro2::Span::call_site()),
                                arguments: syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                                    colon2_token: None,
                                    lt_token: Token![<](proc_macro2::Span::call_site()),
                                    args: syn::punctuated::Punctuated::from_iter([
                                        syn::GenericArgument::Type(mapped_elem)
                                    ]),
                                    gt_token: Token![>](proc_macro2::Span::call_site()),
                                }),
                            });
                            return syn::Type::Path(syn::TypePath {
                                path: chan_path,
                                qself: None,
                            });
                        }
                    }
                }
                if first_name == "__go_chan" {
                    // Extract element type from generic args: `__go_chan<T>`
                    let seg = &type_path.path.segments[0];
                    if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                        if let Some(syn::GenericArgument::Type(elem_ty)) = args.args.first() {
                            // Map the element type first
                            let mapped_elem = map_go_types(elem_ty);
                            // Build GoChannel<T> with the mapped element type
                            let mut chan_path = syn::Path::from(syn::Ident::new("GoChannel", proc_macro2::Span::call_site()));
                            chan_path.segments.clear();
                            chan_path.segments.push(syn::PathSegment {
                                ident: syn::Ident::new("GoChannel", proc_macro2::Span::call_site()),
                                arguments: syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                                    colon2_token: None,
                                    lt_token: Token![<](proc_macro2::Span::call_site()),
                                    args: syn::punctuated::Punctuated::from_iter([
                                        syn::GenericArgument::Type(mapped_elem)
                                    ]),
                                    gt_token: Token![>](proc_macro2::Span::call_site()),
                                }),
                            });
                            return syn::Type::Path(syn::TypePath {
                                path: chan_path,
                                qself: None,
                            });
                        }
                    }
                }
                if first_name == "__go_map" {
                    // Extract key and value types from `__go_map<K, V>`
                    let seg = &type_path.path.segments[0];
                    if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                        let keys: Vec<_> = args.args.iter().collect();
                        if keys.len() >= 2 {
                            if let syn::GenericArgument::Type(key_ty) = &keys[0] {
                                if let syn::GenericArgument::Type(val_ty) = &keys[1] {
                                    let mapped_key = map_go_types(key_ty);
                                    let mapped_val = map_go_types(val_ty);
                                    // Build gourd::prelude::HashMap<K, V>
                                    let mut map_path = syn::Path::from(syn::Ident::new("gourd", proc_macro2::Span::call_site()));
                                    map_path.segments.push(syn::PathSegment::from(syn::Ident::new("prelude", proc_macro2::Span::call_site())));
                                    map_path.segments.push(syn::PathSegment {
                                        ident: syn::Ident::new("HashMap", proc_macro2::Span::call_site()),
                                        arguments: syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                                            colon2_token: None,
                                            lt_token: Token![<](proc_macro2::Span::call_site()),
                                            args: syn::punctuated::Punctuated::from_iter([
                                                syn::GenericArgument::Type(mapped_key),
                                                syn::GenericArgument::Type(mapped_val),
                                            ]),
                                            gt_token: Token![>](proc_macro2::Span::call_site()),
                                        }),
                                    });
                                    return syn::Type::Path(syn::TypePath {
                                        path: map_path,
                                        qself: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }

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
                        "string" => "GoString",
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
                        "error" => "Box<dyn std::error::Error>",
                        _ => unreachable!(),
                    };
                    // Build the path from the mapped type string.
                    // Handle simple names ("bool", "i32") and qualified paths ("::gourd::GoString").
                    if mapped_ident.contains("::") {
                        // Qualified path: build from segments
                        let (is_global, rest) = if mapped_ident.starts_with("::") {
                            (true, &mapped_ident[2..])
                        } else {
                            (false, mapped_ident)
                        };
                        let mut path_segments = syn::punctuated::Punctuated::new();
                        for segment in rest.split("::").filter(|s| !s.is_empty()) {
                            path_segments.push(syn::PathSegment::from(
                                syn::Ident::new(segment, proc_macro2::Span::call_site()),
                            ));
                        }
                        if is_global {
                            return syn::Type::Path(syn::TypePath {
                                path: syn::Path {
                                    leading_colon: Some(syn::Token![::](proc_macro2::Span::call_site())),
                                    segments: path_segments,
                                },
                                qself: None,
                            });
                        } else {
                            return syn::Type::Path(syn::TypePath {
                                path: syn::Path { segments: path_segments, leading_colon: None },
                                qself: None,
                            });
                        }
                    } else {
                        // Simple identifier
                        let id = syn::Ident::new(mapped_ident, proc_macro2::Span::call_site());
                        return syn::Type::Path(syn::TypePath {
                            path: syn::Path::from(id),
                            qself: None,
                        });
                    };
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

// ─── HIR function and struct types ───────────────────────────────────────────

/// A higher-level function representation that captures the function's
/// semantic intent directly, avoiding token-level manipulation.
#[derive(Clone)]
pub struct HirFunction {
    /// Function name (preserved as camelCase from Go source)
    pub name: syn::Ident,
    /// Function parameters: (name, type) pairs
    pub params: Vec<(syn::Ident, Box<HirType>)>,
    /// Return types (empty if function returns nothing)
    pub returns: Vec<Box<HirType>>,
    /// Function body (block of statements)
    pub body: HirBlock,
}

/// A higher-level struct representation that captures the struct's
/// semantic intent directly, avoiding token-level manipulation.
#[derive(Clone)]
pub struct HirStruct {
    /// Struct name (preserved as camelCase from Go source)
    pub name: syn::Ident,
    /// Struct fields: (name, type) pairs
    pub fields: Vec<(syn::Ident, Box<HirType>)>,
}

impl HirFunction {
    /// Create a new empty HIR function.
    pub fn new(name: syn::Ident) -> Self {
        HirFunction {
            name,
            params: Vec::new(),
            returns: Vec::new(),
            body: HirBlock::new(),
        }
    }
}

impl HirStruct {
    /// Create a new empty HIR struct.
    pub fn new(name: syn::Ident) -> Self {
        HirStruct {
            name,
            fields: Vec::new(),
        }
    }
}

// ─── Helpers for parsing Go input into HIR types ─────────────────────────────
