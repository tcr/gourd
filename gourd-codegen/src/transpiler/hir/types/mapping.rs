//! Go type name and compound type mapping.
//!
//! Maps Go type strings ("int", "[]T", "map[K]V") to HIR types.
//! Also handles parsing of Go struct and interface declarations.

use crate::transpiler::hir::types::primitives::{HirType, HirTypeKind, HirInterfaceMethod};
use proc_macro2::{TokenStream, TokenTree, Delimiter};
use syn::Ident;


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
pub(crate) fn parse_interface_params(input: TokenStream) -> Vec<(Ident, Box<HirType>)> {
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
