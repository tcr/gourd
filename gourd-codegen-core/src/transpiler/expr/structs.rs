//! Struct literal transpilation: `ExprStruct` → `Point { x: 1, y: 2 }`.
//!
//! Go struct literals and Rust struct literals have the same syntax:
//! `Name { field: value }`. The transpiler emits the struct literal
//! as-is, recursively transpiling field expressions.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{ExprStruct, FieldValue, Member};

/// Transpile a Go struct literal `Name{field: value}` → `Name { field: value }`.
///
/// Both Go and Rust use `:` as the field:value separator, so the syntax is
/// already compatible. We just need to recursively transpile field values
/// and handle the optional rest expression (`..Base`).
pub fn transpile_struct(input: &ExprStruct) -> TokenStream {
    let path = super::dispatch::go_to_rust(&syn::Expr::Path(syn::ExprPath {
        attrs: vec![],
        qself: None,
        path: input.path.clone(),
    }));
    let fields: Vec<_> = input
        .fields
        .iter()
        .map(transpile_field)
        .collect();
    let rest = input.rest.as_ref().map(|rest_expr| {
        let rest_rust = super::dispatch::go_to_rust(rest_expr);
        // Always emit a trailing comma before `..rest` to avoid Rust
        // misinterpreting it as a range expression (e.g. `Point{x: 1, ..rest}`
        // is ambiguous — `x: 1,` with trailing comma disambiguates it).
        quote! { , .. #rest_rust }
    });
    quote! { #path { #(#fields),* #rest } }
}

/// Transpile a single struct field value.
///
/// Handles:
/// - Named fields with explicit value: `Field: value` → `Field: value`
/// - Shorthand fields (no colon): `Field` → `Field: Field` (expand shorthand)
/// - Unnamed fields: positional struct literals like `Point{1, 2}`
///   (these get a compile_error since Rust doesn't support positional struct literals)
fn transpile_field(field: &FieldValue) -> TokenStream {
    let member = &field.member;
    match member {
        Member::Named(ident) => {
            // Shorthand: no colon means `Struct { x }` → `Struct { x: x }`
            if field.colon_token.is_none() {
                return quote! { #ident: #ident };
            }
            // Named field with explicit value: `Field: value`
            let value = super::dispatch::go_to_rust(&field.expr);
            quote! { #ident: #value }
        }
        Member::Unnamed(_idx) => {
            // Unnamed/positional field: Go allows `Point{1, 2}`
            // Rust doesn't support positional struct literals, so emit compile_error!
            let value = super::dispatch::go_to_rust(&field.expr);
            quote! { compile_error!("TODO: positional struct fields are not supported; use named fields: Point { x: 1 }") #value }
        }
    }
}
