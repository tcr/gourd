use super::expr::go_to_rust;
use super::parsing::{GoFnInputs, GoFnOutput};
use super::types::map_go_types;
use proc_macro2::TokenStream;
use quote::quote;
use syn::ext::IdentExt;
use syn::parse::{discouraged::Speculative, Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token;
use syn::{Expr, Ident};

use syn::fold::Fold;

/// Receiver parsing: `(name Type)` or `(name *Type)` where * means pointer receiver.
///
/// Implemented as a proper `syn::parse` impl so it works with nested parsing.
pub(crate) struct Receiver {
    pub(crate) name: Ident,
    pub(crate) _ty: syn::Type,
    pub(crate) pointer: bool,  // true for `*Foo` → `&mut self`
}

impl Parse for Receiver {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // First token: optional `*` followed by identifier (name) or just identifier.
        let fork = input.fork();
        let (is_ptr, name) = if fork.peek(syn::token::Star) {
            let _star: syn::token::Star = fork.parse()?;
            let name: Ident = fork.parse()?;
            (true, name)
        } else {
            let name: Ident = fork.parse()?;
            (false, name)
        };

        // Check if the token after name is a type (not `)` or end of input)
        // If it's a type, consume it and the name as a separate identifier.
        // If not, the name IS the type.
        if input.peek(syn::token::Star) {
            // `*Type` pattern (single token with deref prefix)
            let _: syn::token::Star = input.parse()?;
            let ty = input.parse::<syn::Type>()?;
            Ok(Receiver { name, _ty: ty, pointer: true })
        } else if input.peek(syn::Ident) {
            // `name Type` pattern — the first ident was the name, second is type
            // Already consumed name from input via fork;
            // Consume the type from the real input
            let ty = input.parse::<syn::Type>()?;
            Ok(Receiver { name, _ty: ty, pointer: is_ptr })
        } else if is_ptr {
            // Just `*Type` — use a default name
            let ty = input.parse::<syn::Type>()?;
            Ok(Receiver {
                name: Ident::new("recv", proc_macro2::Span::call_site()),
                _ty: ty,
                pointer: true,
            })
        } else {
            // Single identifier — that's the type (receiver name default)
            let ty = input.parse::<syn::Type>()?;
            Ok(Receiver {
                name: Ident::new("recv", proc_macro2::Span::call_site()),
                _ty: ty,
                pointer: is_ptr,
            })
        }
    }
}

/// Convert token stream to Receiver — fallback for callers that already have tokens.
pub(crate) fn receiver_from_tokens(tokens: TokenStream) -> syn::Result<Receiver> {
    syn::parse2::<Receiver>(tokens)
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

        // Convert the receiver tokens to a Receiver struct via proper parsing
        let recv = receiver_from_tokens(recv_paren.parse::<proc_macro2::TokenStream>()?)?;

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
