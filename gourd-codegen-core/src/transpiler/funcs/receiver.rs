//! Receiver replacement logic.
//!
//! Replaces receiver names with `self` in Go expressions.

use syn::fold::Fold;
use syn::{Expr, Ident};

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
            other => syn::fold::fold_expr(self, other),
        }
    }

    fn fold_local(&mut self, local: syn::Local) -> syn::Local {
        syn::fold::fold_local(self, local)
    }
}
