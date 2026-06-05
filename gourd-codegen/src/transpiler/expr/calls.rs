//! Call and method transpilation: `Call`, `MethodCall`, `Field`, `Index`.
//! Also handles Rust macros (`Expr::Macro`) and method-call shorthand.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, ExprField, ExprIndex, ExprMacro, ExprMethodCall};

use super::dispatch::emit_todo;

pub fn transpile_call(input: &syn::ExprCall) -> TokenStream {
    let args: Vec<_> = input.args.iter().map(super::dispatch::go_to_rust).collect();

    // Handle Go builtin functions that are now stdlib: copy, delete
    if let Expr::Path(path) = &*input.func {
        if let Some(_func_name) = try_parse_std_copy(path) {
            let func = args.iter().enumerate().map(|(i, arg)| {
                if i == 0 { quote! { &mut #arg } } else { quote! { & #arg } }
            }).collect::<Vec<_>>();
            return quote! { ::gourd::prelude::std_copy( #(#func),* ) };
        }
        if let Some(_func_name) = try_parse_std_delete(path) {
            return quote! { ::gourd::prelude::std_delete( #(#args),* ) };
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
            "min" => {
                if args.len() == 2 {
                    return quote! { ::gourd::prelude::min( #(#args),* ) };
                }
            }
            "max" => {
                if args.len() == 2 {
                    return quote! { ::gourd::prelude::max( #(#args),* ) };
                }
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
    // Go `append(slice, items...)` → stdlib std_append
    // Stdlib version: converts slice to Vec, extends with items, returns new Vec.
    if let Expr::Path(path) = &*input.func
        && let Some(_func_name) = try_parse_std_append(path)
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
        // append(slice, items...) — pass items as a slice
        let items: Vec<_> = append_args[1..].iter()
            .map(|arg| super::dispatch::go_to_rust(arg))
            .collect();
        return quote! { ::gourd::prelude::std_append( #slice, &[ #(#items),* ] ) };
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
            if type_str.contains("chan") || type_str.contains("__go_chan") {
                if make_args.len() == 2 {
                    return quote! { GoChannel::<#type_tokens>::new() };
                } else {
                    let cap = super::dispatch::go_to_rust(&make_args[1]);
                    return quote! { GoChannel::<#type_tokens>::with_capacity(#cap) };
                }
            }
            if type_str.contains("map[") {
                return quote! { ::std::collections::HashMap::new() };
            }
            if type_str.starts_with("[]") {
                let type_default: TokenStream = quote! { #type_tokens::default() };
                if make_args.len() == 2 {
                    let len = super::dispatch::go_to_rust(&make_args[1]);
                    return quote! { ::std::iter::repeat(#type_default).take(#len).collect::<#type_tokens>() };
                } else {
                    let len = super::dispatch::go_to_rust(&make_args[1]);
                    return quote! { ::std::iter::repeat(#type_default).take(#len).collect::<#type_tokens>() };
                }
            }
            return emit_todo("unsupported type in make()");
        } else {
            return emit_todo("make() requires at least a type argument");
        }
    }
    // Go `strings` package top-level functions
    if let Expr::Path(path) = &*input.func
        && let Some(func_name) = try_parse_strings_call(path)
    {
        match func_name.as_str() {
            "Replace" => return quote! { ::gourd::prelude::strings_replace( #(#args),* ) },
            "ReplaceAll" => return quote! { ::gourd::prelude::strings_replace_all( #(#args),* ) },
            "HasPrefix" => return quote! { ::gourd::prelude::has_prefix( #(#args),* ) },
            "HasSuffix" => return quote! { ::gourd::prelude::has_suffix( #(#args),* ) },
            "Contains" => return quote! { ::gourd::prelude::contains_str( #(#args),* ) },
            "Split" => return quote! { ::gourd::prelude::split( #(#args),* ) },
            "Join" => return quote! { ::gourd::prelude::join( #(#args),* ) },
            "Index" => return quote! { ::gourd::prelude::index_str( #(#args),* ) },
            "LastIndex" => return quote! { ::gourd::prelude::last_index_str( #(#args),* ) },
            "Trim" => return quote! { ::gourd::prelude::trim( #(#args),* ) },
            "TrimLeft" => return quote! { ::gourd::prelude::trim_left( #(#args),* ) },
            "TrimRight" => return quote! { ::gourd::prelude::trim_right( #(#args),* ) },
            "ToUpper" => return quote! { ::gourd::prelude::to_upper( #(#args),* ) },
            "ToLower" => return quote! { ::gourd::prelude::to_lower( #(#args),* ) },
            "Repeat" => return quote! { ::gourd::prelude::repeat( #(#args),* ) },
            "Fields" => return quote! { ::gourd::prelude::fields( #(#args),* ) },
            _ => return emit_todo(&format!("strings.{}()", func_name)),
        }
    }
    // Go `os` package top-level functions
    if let Expr::Path(path) = &*input.func
        && let Some(func_name) = try_parse_os_call(path)
    {
        match func_name.as_str() {
            "Open" => return quote! { ::gourd::prelude::os_open( #(#args),* ) },
            "ReadFile" => return quote! { ::gourd::prelude::os_read_file( #(#args),* ) },
            "WriteFile" => return quote! { ::gourd::prelude::os_write_file( #(#args),* ) },
            "Mkdir" => return quote! { ::gourd::prelude::os_mkdir( #(#args),* ) },
            "MkdirAll" => return quote! { ::gourd::prelude::os_mkdir_all( #(#args),* ) },
            "Remove" => return quote! { ::gourd::prelude::os_remove( #(#args),* ) },
            "Chdir" => return quote! { ::gourd::prelude::os_chdir( #(#args),* ) },
            "Getenv" => return quote! { ::gourd::prelude::os_getenv( #(#args),* ) },
            "Setenv" => return quote! { ::gourd::prelude::os_setenv( #(#args),* ) },
            _ => return emit_todo(&format!("os.{func_name}()")),
        }
    }
    // Go `io` package top-level functions
    if let Expr::Path(path) = &*input.func
        && let Some(func_name) = try_parse_io_call(path)
    {
        match func_name.as_str() {
            "Copy" => return quote! { ::gourd::prelude::io_copy( #(#args),* ) },
            "ReadAll" => return quote! { ::gourd::prelude::io_read_all( #(#args),* ) },
            _ => return emit_todo(&format!("io.{func_name}()")),
        }
    }
    // Go `bytes` package top-level functions
    if let Expr::Path(path) = &*input.func
        && let Some(func_name) = try_parse_bytes_call(path)
    {
        match func_name.as_str() {
            "Contains" => return quote! { ::gourd::prelude::bytes_contains( #(#args),* ) },
            "HasPrefix" => return quote! { ::gourd::prelude::bytes_has_prefix( #(#args),* ) },
            "HasSuffix" => return quote! { ::gourd::prelude::bytes_has_suffix( #(#args),* ) },
            "Index" => return quote! { ::gourd::prelude::bytes_index( #(#args),* ) },
            "Split" => return quote! { ::gourd::prelude::bytes_split( #(#args),* ) },
            "Join" => return quote! { ::gourd::prelude::bytes_join( #(#args),* ) },
            "Replace" => return quote! { bytes_replace( #(#args),* ) },
            _ => return emit_todo(&format!("bytes.{func_name}()")),
        }
    }
    // Go `encoding/json` package top-level functions
    if let Expr::Path(path) = &*input.func
        && let Some(func_name) = try_parse_json_call(path)
    {
        match func_name.as_str() {
            "Marshal" => return quote! { ::gourd::prelude::json_marshal( #(#args),* ) },
            "Unmarshal" => return quote! { ::gourd::prelude::json_unmarshal( #(#args),* ) },
            _ => return emit_todo(&format!("json.{func_name}()")),
        }
    }
    // Go `time` package top-level functions
    if let Expr::Path(path) = &*input.func
        && let Some(func_name) = try_parse_time_call(path)
    {
        match func_name.as_str() {
            "Now" => return quote! { ::gourd::prelude::time_now( #(#args),* ) },
            "Since" => return quote! { ::gourd::prelude::time_since( #(#args),* ) },
            "Until" => return quote! { ::gourd::prelude::time_until( #(#args),* ) },
            "Sleep" => return quote! { ::gourd::prelude::time_sleep( #(#args),* ) },
            _ => return emit_todo(&format!("time.{func_name}()")),
        }
    }
    // Default: regular function call
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
    let seq_str = quote! { #seq }.to_string();
    let idx = super::dispatch::go_to_rust(&input.index);
    // Delegate HashMap reads to prelude: `::gourd::prelude::map_get(m, k)`.
    if seq_str.contains("HashMap") || seq_str.contains("hash_map") {
        return quote! { ::gourd::prelude::map_get( #seq, #idx ) };
    }
    // Check if the index is a string literal — for maps, use .get() instead.
    if let Expr::Lit(lit) = &*input.index
        && matches!(&lit.lit, syn::Lit::Str(_))
    {
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
    // Check if this is a package function call: `strings.Replace(...)`, etc.
    if let Expr::Path(path) = &*input.receiver {
        let pkg = path.path.get_ident().map(|i| i.to_string());
        if let Some(pkg) = pkg {
            match pkg.as_str() {
                "strings" => {
                    return match method_name.to_string().as_str() {
                        "Replace" => quote! { ::gourd::prelude::strings_replace( #(#args),* ) },
                        "ReplaceAll" => quote! { ::gourd::prelude::strings_replace_all( #(#args),* ) },
                        "HasPrefix" => quote! { ::gourd::prelude::has_prefix( #(#args),* ) },
                        "HasSuffix" => quote! { ::gourd::prelude::has_suffix( #(#args),* ) },
                        "Contains" => quote! { ::gourd::prelude::contains_str( #(#args),* ) },
                        "Split" => quote! { ::gourd::prelude::split( #(#args),* ) },
                        "Join" => {
                            let elems = &args[0];
                            let rest = &args[1..];
                            // elems is already a vec![...] from transpile_array
                            quote! { ::gourd::prelude::join( #elems, #(#rest),* ) }
                        },
                        "Index" => quote! { ::gourd::prelude::index_str( #(#args),* ) },
                        "LastIndex" => quote! { ::gourd::prelude::last_index_str( #(#args),* ) },
                        "Trim" => quote! { ::gourd::prelude::trim( #(#args),* ) },
                        "TrimLeft" => quote! { ::gourd::prelude::trim_left( #(#args),* ) },
                        "TrimRight" => quote! { ::gourd::prelude::trim_right( #(#args),* ) },
                        "ToUpper" => quote! { ::gourd::prelude::to_upper( #(#args),* ) },
                        "ToLower" => quote! { ::gourd::prelude::to_lower( #(#args),* ) },
                        "Repeat" => quote! { ::gourd::prelude::repeat( #(#args),* ) },
                        "Fields" => quote! { ::gourd::prelude::fields( #(#args),* ) },
                        _ => emit_todo(&format!("strings.{method_name}()")),
                    };
                }
                "os" => {
                    return match method_name.to_string().as_str() {
                        "Open" => quote! { ::gourd::prelude::os_open( #(#args),* ) },
                        "ReadFile" => quote! { ::gourd::prelude::os_read_file( #(#args),* ) },
                        "WriteFile" => quote! { ::gourd::prelude::os_write_file( #(#args),* ) },
                        "Mkdir" => quote! { ::gourd::prelude::os_mkdir( #(#args),* ) },
                        "MkdirAll" => quote! { ::gourd::prelude::os_mkdir_all( #(#args),* ) },
                        "Remove" => quote! { ::gourd::prelude::os_remove( #(#args),* ) },
                        "Chdir" => quote! { ::gourd::prelude::os_chdir( #(#args),* ) },
                        "Getenv" => quote! { ::gourd::prelude::os_getenv( #(#args),* ) },
                        "Setenv" => quote! { ::gourd::prelude::os_setenv( #(#args),* ) },
                        _ => emit_todo(&format!("os.{method_name}()")),
                    };
                }
                "io" => {
                    return match method_name.to_string().as_str() {
                        "Copy" => quote! { ::gourd::prelude::io_copy( #(#args),* ) },
                        "ReadAll" => quote! { ::gourd::prelude::io_read_all( #(#args),* ) },
                        _ => emit_todo(&format!("io.{method_name}()")),
                    };
                }
                "bytes" => {
                    return match method_name.to_string().as_str() {
                        "Contains" => quote! { ::gourd::prelude::bytes_contains( #(#args),* ) },
                        "HasPrefix" => quote! { ::gourd::prelude::bytes_has_prefix( #(#args),* ) },
                        "HasSuffix" => quote! { ::gourd::prelude::bytes_has_suffix( #(#args),* ) },
                        "Index" => quote! { ::gourd::prelude::bytes_index( #(#args),* ) },
                        "Split" => quote! { ::gourd::prelude::bytes_split( #(#args),* ) },
                        "Join" => quote! { ::gourd::prelude::bytes_join( #(#args),* ) },
                        "Replace" => quote! { bytes_replace( #(#args),* ) },
                        _ => emit_todo(&format!("bytes.{method_name}()")),
                    };
                }
                "json" => {
                    return match method_name.to_string().as_str() {
                        "Marshal" => quote! { ::gourd::prelude::json_marshal( #(#args),* ) },
                        "Unmarshal" => quote! { ::gourd::prelude::json_unmarshal( #(#args),* ) },
                        _ => emit_todo(&format!("json.{method_name}()")),
                    };
                }
                "time" => {
                    return match method_name.to_string().as_str() {
                        "Now" => quote! { ::gourd::prelude::time_now() },
                        "Since" => quote! { ::gourd::prelude::time_since( #(#args),* ) },
                        "Until" => quote! { ::gourd::prelude::time_until( #(#args),* ) },
                        "Sleep" => quote! { ::gourd::prelude::time_sleep( #(#args),* ) },
                        _ => emit_todo(&format!("time.{method_name}()")),
                    };
                }
                _ => {}
            }
        }
    }
    quote! { #receiver.#method_name( #(#args),* ) }
}

pub fn transpile_field(input: &ExprField) -> TokenStream {
    // Check for `fmt.Sprintf`, `fmt.Print`, `fmt.Println`, `fmt.Printf`
    if let Some(rust_fn) = try_parse_fmt_field(&input.base, &input.member) {
        return rust_fn;
    }
    let base = super::dispatch::go_to_rust(&input.base);
    let field = &input.member;
    quote! { #base.#field }
}

/// Try to parse strings package function calls: `strings.Replace(...)`, etc.
fn try_parse_strings_call(path: &syn::ExprPath) -> Option<String> {
    if path.path.segments.len() != 2 {
        return None;
    }
    let pkg = path.path.segments[0].ident.to_string();
    if pkg != "strings" {
        return None;
    }
    Some(path.path.segments[1].ident.to_string())
}

/// Try to parse os package function calls: `os.Open(...)`, etc.
fn try_parse_os_call(path: &syn::ExprPath) -> Option<String> {
    if path.path.segments.len() != 2 {
        return None;
    }
    let pkg = path.path.segments[0].ident.to_string();
    if pkg != "os" {
        return None;
    }
    Some(path.path.segments[1].ident.to_string())
}

/// Try to parse `io` package function calls: `io.Copy(...)`, etc.
fn try_parse_io_call(path: &syn::ExprPath) -> Option<String> {
    if path.path.segments.len() != 2 {
        return None;
    }
    let pkg = path.path.segments[0].ident.to_string();
    if pkg != "io" {
        return None;
    }
    Some(path.path.segments[1].ident.to_string())
}

/// Try to parse `bytes` package function calls: `bytes.Contains(...)`, etc.
fn try_parse_bytes_call(path: &syn::ExprPath) -> Option<String> {
    if path.path.segments.len() != 2 {
        return None;
    }
    let pkg = path.path.segments[0].ident.to_string();
    if pkg != "bytes" {
        return None;
    }
    Some(path.path.segments[1].ident.to_string())
}

/// Try to parse `encoding/json` package function calls: `json.Marshal(...)`, etc.
fn try_parse_json_call(path: &syn::ExprPath) -> Option<String> {
    if path.path.segments.len() != 2 {
        return None;
    }
    let pkg = path.path.segments[0].ident.to_string();
    if pkg != "json" {
        return None;
    }
    Some(path.path.segments[1].ident.to_string())
}

/// Try to parse `time` package function calls: `time.Now()`, etc.
fn try_parse_time_call(path: &syn::ExprPath) -> Option<String> {
    if path.path.segments.len() != 2 {
        return None;
    }
    let pkg = path.path.segments[0].ident.to_string();
    if pkg != "time" {
        return None;
    }
    Some(path.path.segments[1].ident.to_string())
}

/// Try to parse `fmt.Sprintf`, `fmt.Print`, etc. from a field access.
fn try_parse_fmt_field(base: &syn::Expr, field: &syn::Member) -> Option<TokenStream> {
    let base_ident = match base {
        syn::Expr::Path(p) => {
            if p.path.segments.len() == 1 && p.path.segments[0].ident == "fmt" {
                Some(p.path.segments[0].ident.to_string())
            } else {
                None
            }
        }
        _ => None,
    };
    if base_ident.is_none() {
        return None;
    }
    let field_name = match field {
        syn::Member::Named(ident) => ident.to_string(),
        syn::Member::Unnamed(idx) => {
            let _ = idx;
            return None;
        }
    };
    match field_name.as_str() {
        "Sprintf" => Some(quote! { ::gourd::prelude::fmt_sprintf }),
        "Print" => Some(quote! { ::gourd::prelude::fmt_print }),
        "Println" => Some(quote! { ::gourd::prelude::fmt_println }),
        "Printf" => Some(quote! { ::gourd::prelude::fmt_printf }),
        _ => None,
    }
}

/// Try to parse `std.copy(...)` function calls.
fn try_parse_std_copy(path: &syn::ExprPath) -> Option<String> {
    if path.path.segments.len() != 2 {
        return None;
    }
    let pkg = path.path.segments[0].ident.to_string();
    if pkg != "std" {
        return None;
    }
    let func = path.path.segments[1].ident.to_string();
    if func == "copy" {
        Some(func)
    } else {
        None
    }
}

/// Try to parse `std.delete(...)` function calls.
fn try_parse_std_delete(path: &syn::ExprPath) -> Option<String> {
    if path.path.segments.len() != 2 {
        return None;
    }
    let pkg = path.path.segments[0].ident.to_string();
    if pkg != "std" {
        return None;
    }
    let func = path.path.segments[1].ident.to_string();
    if func == "delete" {
        Some(func)
    } else {
        None
    }
}

/// Try to parse `std.append(...)` function calls.
fn try_parse_std_append(path: &syn::ExprPath) -> Option<String> {
    if path.path.segments.len() != 2 {
        return None;
    }
    let pkg = path.path.segments[0].ident.to_string();
    if pkg != "std" {
        return None;
    }
    let func = path.path.segments[1].ident.to_string();
    if func == "append" {
        Some(func)
    } else {
        None
    }
}


