//! Receiver parsing: `(name Type)` or `(name *Type)` where * means pointer receiver.
//!
//! Parses Go-style receiver declarations and builds the AST for `impl` blocks.

use super::parsing::{GoFnInputs, GoFnOutput};
use syn::ext::IdentExt;
use syn::parse::discouraged::Speculative;
use syn::parse::{Parse, ParseStream};
use syn::token;
use syn::{Expr, Ident};

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
        // Go receiver patterns:
        //   (c Type)          — value receiver
        //   (c *Type)         — pointer receiver with name
        //   (*Type)           — pointer receiver without name
        //   (Type)            — value receiver without name

        // Peek at first token to distinguish patterns.
        let fork = input.fork();
        if fork.peek(syn::token::Star) {
            // `*Type` pattern — pointer receiver, no separate name
            let _: syn::token::Star = input.parse()?;
            let ty = input.parse::<syn::Type>()?;
            Ok(Receiver {
                name: Ident::new("recv", proc_macro2::Span::call_site()),
                _ty: ty,
                pointer: true,
            })
        } else if fork.peek(syn::token::Paren) {
            // Parenthesized receiver — shouldn't happen inside receiver parsing,
            // but handle gracefully
            Ok(Receiver {
                name: Ident::new("recv", proc_macro2::Span::call_site()),
                _ty: input.parse::<syn::Type>()?,
                pointer: false,
            })
        } else {
            // `name Type` or `name *Type` pattern
            let name: Ident = input.parse()?;
            if input.peek(syn::token::Star) {
                // `name *Type` — pointer receiver with explicit name
                let _: syn::token::Star = input.parse()?;
                let ty = input.parse::<syn::Type>()?;
                Ok(Receiver { name, _ty: ty, pointer: true })
            } else if input.peek(syn::Ident) {
                // `name Type` — value receiver with name
                let ty = input.parse::<syn::Type>()?;
                Ok(Receiver { name, _ty: ty, pointer: false })
            } else {
                // Just `name` — name IS the type
                Ok(Receiver {
                    name: name.clone(),
                    _ty: syn::Type::Path(syn::TypePath {
                        path: syn::Path::from(name),
                        qself: None,
                    }),
                    pointer: false,
                })
            }
        }
    }
}

/// A receiver function: `func (recv Type) name(params) output { body }`
pub(crate) struct ReceiverFn {
    pub(crate) recv: Receiver,
    pub(crate) ident: Ident,
    pub(crate) inputs: GoFnInputs,
    pub(crate) output: Option<GoFnOutput>,
    /// Parsed body expressions (converted to Rust via `go_to_rust`).
    pub(crate) stmts: Vec<Expr>,
}

impl Parse for ReceiverFn {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _fn_kw: Ident = input.call(Ident::parse_any)?;

        // Parse `(receiver)` — this is a Parenthesis Group
        let recv_paren;
        let _paren = syn::parenthesized!(recv_paren in input);

        // Parse receiver from the parenthesized tokens
        let recv = syn::parse2(recv_paren.parse::<proc_macro2::TokenStream>()?)?;

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

        // Parse body: parse expressions one at a time from the brace content,
        // optionally consuming trailing semicolons.
        let brace_content;
        let _brace = syn::braced!(brace_content in input);

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
                    stmts.push(expr);
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
