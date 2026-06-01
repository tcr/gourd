//! Receiver function output generation.
//!
//! Converts parsed `ReceiverFn` AST into Rust `impl` block tokens.

use super::expr::go_to_rust;
use super::receiver::{Receiver, ReceiverFn};
use super::types::map_go_types;
use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::token;
use syn::{Expr, ExprAssign, Ident};

use syn::fold::Fold;

/// Transpile a receiver function to Rust: `impl Struct { fn method(...) { ... } }`
pub fn go_to_rust_receiver_fn(input: TokenStream) -> TokenStream {
    match syn::parse2::<ReceiverFn>(input) {
        Ok(parsed) => {
            let Receiver { name: recv_name, _ty: struct_ty, pointer } = parsed.recv;
            let fn_name = &parsed.ident;
            let generics = Punctuated::<syn::GenericParam, token::Comma>::new();

            // Receiver: `&mut self` for pointer, `&self` for value
            let self_arg = if pointer {
                quote! { &mut self }
            } else {
                quote! { &self }
            };

            // Map parameters (reuse GoFnInputs logic)
            let mut all_params = Vec::<TokenStream>::new();
            for param in &parsed.inputs.args {
                let id = &param.id;
                if let Some(_ty) = &param.ty {
                    match (&param.ty, &param.slice_elem) {
                        (Some(ty), None) => {
                            let mapped = map_go_types(ty);
                            all_params.push(quote! { #id: #mapped });
                        }
                        (Some(_ty), Some(slice_inner)) => {
                            let mapped = map_go_types(slice_inner);
                            all_params.push(quote! { #id: &[ #mapped ]});
                        }
                        _ => {}
                    }
                } else {
                    all_params.push(quote! { #id });
                }
            }

            // Map output
            let output = parsed.output.as_ref().map(|output| {
                if output.tys.is_empty() {
                    quote! {}
                } else {
                    let mapped: Vec<_> = output.tys.iter().map(map_go_types).collect();
                    match mapped.len() {
                        1 => {
                            let m = &mapped[0];
                            quote! { -> #m }
                        }
                        _ => quote! { -> ( #(#mapped),* ) },
                    }
                }
            }).unwrap_or_else(|| quote! {});

            // Transpile the body: For each statement, first rename the receiver
            // to "self" in the Go AST, then transpile to Rust via go_to_rust.
            let mut stmts: Vec<TokenStream> = Vec::new();
            for expr in &parsed.stmts {
                let renamed = replace_receiver(expr.clone(), &recv_name);
                let transpiled = go_to_rust(&renamed);
                stmts.push(transpiled);
            }

            let body: Box<syn::ExprBlock> = syn::parse_quote!({ #(#stmts);* });

            // Build the parameter list: if there are additional params, emit
            // `self_arg, ...params`. Otherwise just `self_arg` (no trailing
            // comma — `fn get(&self,)` is not valid Rust).
            let param_list = if all_params.is_empty() {
                quote! { #self_arg }
            } else {
                quote! { #self_arg, #(#all_params),* }
            };
            quote! {
                impl #generics #struct_ty {
                    fn #fn_name (#param_list) #output #body
                }
            }
        }
        Err(e) => e.to_compile_error(),
    }
}

/// Replace receiver name with `self` in a Go expression.
///
/// Uses `syn::fold::Fold` to walk the AST tree and replace:
/// - `recv_name` → `self` (path)
/// - `recv_name.field` → `self.field` (field access)
/// - `recv_name.method()` → `self.method()` (method call)
pub(crate) fn replace_receiver(expr: Expr, recv_name: &Ident) -> Expr {
    let mut replacer = ReceiverReplacer {
        recv_name: recv_name.clone(),
    };
    replacer.fold_expr(expr)
}

/// A `syn::fold::Fold` visitor that replaces the receiver name with `self`.
struct ReceiverReplacer {
    recv_name: Ident,
}

impl Fold for ReceiverReplacer {
    fn fold_expr(&mut self, expr: Expr) -> Expr {
        match expr {
            Expr::Path(p) => {
                if p.path.is_ident(&self.recv_name) {
                    // recv → self
                    Expr::Path(syn::ExprPath {
                        attrs: Vec::new(),
                        qself: None,
                        path: syn::Path::from(Ident::new("self", proc_macro2::Span::call_site())),
                    })
                } else {
                    // Path doesn't match — recurse into it via default impl
                    Expr::Path(syn::ExprPath {
                        attrs: p.attrs,
                        qself: p.qself,
                        path: syn::fold::fold_path(self, p.path),
                    })
                }
            }
            Expr::Field(f) => {
                // Check if base is recv_name → self.member
                let new_base = if let Expr::Path(base_path) = &*f.base
                    && base_path.path.is_ident(&self.recv_name)
                {
                    // recv.field → self.field
                    Box::new(syn::Expr::Path(syn::ExprPath {
                        attrs: Vec::new(),
                        qself: None,
                        path: syn::Path::from(Ident::new("self", proc_macro2::Span::call_site())),
                    }))
                } else {
                    // Base doesn't match — recurse into it
                    Box::new(self.fold_expr(*f.base))
                };
                Expr::Field(syn::ExprField {
                    attrs: Vec::new(),
                    base: new_base,
                    dot_token: f.dot_token,
                    member: f.member,
                })
            }
            Expr::Assign(assign) => {
                // Handle struct field assignment: `f.x = val` → `self.x = val`
                let new_left = self.fold_expr(*assign.left);
                let new_right = self.fold_expr(*assign.right);
                Expr::Assign(ExprAssign {
                    attrs: Vec::new(),
                    left: Box::new(new_left),
                    eq_token: assign.eq_token,
                    right: Box::new(new_right),
                })
            }
            other => syn::fold::fold_expr(self, other),
        }
    }

    fn fold_local(&mut self, local: syn::Local) -> syn::Local {
        syn::fold::fold_local(self, local)
    }
}
