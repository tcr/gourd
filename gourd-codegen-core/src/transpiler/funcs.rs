use super::expr::go_to_rust;
use super::parsing::{GoFnInputs, GoFnOutput};
use super::types::map_go_types;
use proc_macro2::TokenStream;
use quote::quote;
use syn::ext::IdentExt;
use syn::parse::discouraged::Speculative;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token;
use syn::{Expr, Ident};

use syn::fold::Fold;

/// Receiver parsing: (name Type) or (name *Type) where * means pointer receiver
pub(crate) struct Receiver {
    pub(crate) name: Ident,
    pub(crate) _ty: syn::Type,
    pub(crate) pointer: bool,  // true for `*Foo` → `&mut self`
}

impl Receiver {
    pub(crate) fn from_tokens(tokens: TokenStream) -> syn::Result<Self> {
        let text: String = tokens.to_string();
        let words: Vec<&str> = text.split_whitespace().collect();

        match words.len() {
            1 => {
                let (name, is_ptr, type_str) = if words[0].starts_with('*') {
                    ("recv", true, &words[0][1..])
                } else {
                    ("recv", false, words[0])
                };
                let ty = syn::parse_str::<syn::Type>(type_str)?;
                let name = Ident::new(name, proc_macro2::Span::call_site());
                Ok(Receiver { name, _ty: ty, pointer: is_ptr })
            }
            2 => {
                let name = Ident::new(words[0], proc_macro2::Span::call_site());
                let is_ptr = words[1].starts_with('*');
                let type_str = if is_ptr { &words[1][1..] } else { words[1] };
                let ty = syn::parse_str::<syn::Type>(type_str)?;
                Ok(Receiver { name, _ty: ty, pointer: is_ptr })
            }
            3 => {
                if words[1] == "*" {
                    let name = Ident::new(words[0], proc_macro2::Span::call_site());
                    let type_str = words[2];
                    let ty = syn::parse_str::<syn::Type>(type_str)?;
                    Ok(Receiver { name, _ty: ty, pointer: true })
                } else {
                    Ok(Receiver { name: Ident::new("recv", proc_macro2::Span::call_site()), _ty: syn::parse_str("unknown").ok().unwrap_or_else(|| syn::Type::Path(syn::TypePath { path: syn::Path::from(Ident::new("unknown", proc_macro2::Span::call_site())), qself: None })), pointer: false })
                }
            }
            _ => Ok(Receiver { name: Ident::new("recv", proc_macro2::Span::call_site()), _ty: syn::parse_str("unknown").ok().unwrap_or_else(|| syn::Type::Path(syn::TypePath { path: syn::Path::from(Ident::new("unknown", proc_macro2::Span::call_site())), qself: None })), pointer: false }),
        }
    }
}

/// A receiver function: `func (recv Type) name(params) output { body }`
pub(crate) struct ReceiverFn {
    pub(crate) recv: Receiver,
    pub(crate) ident: Ident,
    pub(crate) inputs: GoFnInputs,
    pub(crate) output: Option<GoFnOutput>,
    /// Parsed body statements as Go AST elements (single tree per statement)
    pub(crate) stmts: Vec<GoStmt>,
}

/// A Go statement (expression or local declaration)
pub(crate) enum GoStmt {
    Expr(Expr),
}

impl Parse for ReceiverFn {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _fn_kw: Ident = input.call(Ident::parse_any)?;

        // Parse `(receiver)` — this is a Parenthesis Group
        let recv_paren;
        let _paren = syn::parenthesized!(recv_paren in input);

        // Convert the receiver tokens to a Receiver struct
        let recv = Receiver::from_tokens(recv_paren.parse::<proc_macro2::TokenStream>()?)?;

        // Parse function name
        let ident: Ident = input.parse()?;

        // Parse parameters (still in parenthesized group)
        let param_paren;
        let _paren2 = syn::parenthesized!(param_paren in input);
        let inputs: GoFnInputs = param_paren.parse()?;

        // Parse optional return type
        let output = if !input.is_empty() && !input.peek(syn::token::Brace) {
            if input.peek(syn::token::RArrow) {
                let _: syn::token::RArrow = input.parse()?;
            }
            Some(input.parse::<GoFnOutput>()?)
        } else {
            None
        };

        // Parse body: parse as a block with no semicolons, split by newlines,
        // parse each statement individually using speculative parsing.
        let brace_content;
        let _brace = syn::braced!(brace_content in input);

        // Parse Go-style: no semicolons required between statements.
        // We parse expressions one at a time from the brace content,
        // optionally consuming a trailing semicolon.
        let mut stmts = Vec::new();
        while !brace_content.is_empty() {
            // Speculatively try to parse a syn::Expr (covers field, binary,
            // unary, call, paren, let ":=", assign, return, etc.)
            let fork = brace_content.fork();
            match fork.parse::<Expr>() {
                Ok(expr) => {
                    brace_content.advance_to(&fork);
                    // Consume optional semicolon
                    if brace_content.peek(token::Semi) {
                        let _semi: token::Semi = brace_content.parse()?;
                    }
                    stmts.push(GoStmt::Expr(expr));
                }
                Err(_) => {
                    // Can't parse anything — error
                    return Err(brace_content.error("expected Go statement (expression or local declaration)"));
                }
            }
        }

        Ok(ReceiverFn { recv, ident, inputs, output, stmts })
    }
}

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
            for stm in &parsed.stmts {
                match stm {
                    GoStmt::Expr(expr) => {
                        let renamed = replace_receiver(expr.clone(), &recv_name);
                        let transpiled = go_to_rust(&renamed);
                        stmts.push(transpiled);
                    }
                }
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
            other => syn::fold::fold_expr(self, other),
        }
    }

    fn fold_local(&mut self, local: syn::Local) -> syn::Local {
        syn::fold::fold_local(self, local)
    }
}


// Old match body removed — replaced by ReceiverReplacer using syn::fold::Fold
