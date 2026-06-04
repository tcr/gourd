//! Receiver parsing: `(name Type)` or `(name *Type)` where * means pointer receiver.
//!
//! Parses Go-style receiver declarations and builds the AST for `impl` blocks.

use super::parsing::{GoFnInputs, GoFnOutput};
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Ident};

/// Receiver parsing: `(name Type)` or `(name *Type)` where * means pointer receiver.
pub(crate) struct Receiver {
    pub(crate) name: Ident,
    pub(crate) _ty: syn::Type,
    pub(crate) pointer: bool,
}

impl Parse for Receiver {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Peek at the first token by forking
        let fork = input.fork();
        
        // First, try to parse a name (identifier)
        let name: Ident = fork.parse()?;
        
        // After the name, check if the next token is * (pointer) or a type
        let fork = input.fork();
        // Skip the name (which we already parsed)
        let _ = fork.parse::<Ident>();
        // Now fork is at the next token
        // Try to parse a TokenTree
        let next_tt: proc_macro2::TokenTree = fork.parse()?;
        
        match next_tt {
            proc_macro2::TokenTree::Punct(p) if p.as_char() == '*' => {
                // `name *Type` pattern — pointer receiver
                // Re-parse from real input: the name was already consumed by the fork check
                // but not by the real input, so we need to re-parse
                // Actually, the name was parsed from fork, not from real input.
                // Real input is still at name.
                // So we need: consume name, then *, then type
                let _: Ident = input.parse()?; // consume name from real input
                let _: proc_macro2::Punct = input.parse()?; // consume *
                let ty = input.parse::<syn::Type>()?;
                Ok(Receiver { name, _ty: ty, pointer: true })
            }
            _ => {
                // `name Type` pattern — value receiver
                // Fork already parsed name, so real input is at name
                let _: Ident = input.parse()?; // consume name from real input
                let ty = input.parse::<syn::Type>()?;
                Ok(Receiver { name, _ty: ty, pointer: false })
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
    pub(crate) stmts: Vec<Expr>,
}

impl Parse for ReceiverFn {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _fn_kw: Ident = input.call(Ident::parse_any)?;

        // Parse receiver group — a Group token with Parenthesis delimiter
        let recv_tt: proc_macro2::TokenTree = input.parse()?;
        let recv: Receiver = match recv_tt {
            proc_macro2::TokenTree::Group(g) => {
                if g.delimiter() == proc_macro2::Delimiter::Parenthesis {
                    syn::parse2(g.stream())?
                } else {
                    return Err(input.error("expected receiver group `(recv Type)`"));
                }
            }
            _ => return Err(input.error("expected receiver group `(recv Type)`")),
        };

        // Parse function name
        let ident: Ident = input.parse()?;

        // Parse parameter group — a Group token with Parenthesis delimiter
        let param_tt: proc_macro2::TokenTree = input.parse()?;
        let inputs: GoFnInputs = match param_tt {
            proc_macro2::TokenTree::Group(g) => {
                if g.delimiter() == proc_macro2::Delimiter::Parenthesis {
                    syn::parse2(g.stream())?
                } else {
                    return Err(input.error("expected parameter group `(params)`"));
                }
            }
            _ => return Err(input.error("expected parameter group `(params)`")),
        };

        // Parse optional return type
        let output = if !input.is_empty() && !input.peek(syn::token::Brace) {
            if input.peek(syn::token::RArrow) {
                let _: syn::token::RArrow = input.parse()?;
            }
            Some(input.parse::<GoFnOutput>()?)
        } else {
            None
        };

        // Parse body — a Group token with Brace delimiter
        let body_tt: proc_macro2::TokenTree = input.parse()?;
        let body: proc_macro2::TokenStream = match body_tt {
            proc_macro2::TokenTree::Group(g) => {
                if g.delimiter() == proc_macro2::Delimiter::Brace {
                    g.stream()
                } else {
                    return Err(input.error("expected body `{`"));
                }
            }
            _ => return Err(input.error("expected body `{`")),
        };

        // Parse body expressions from the Group content
        let mut stmts = Vec::new();
        let body_str = body.to_string();
        
        // The body tokens are: `b.value = b.value + z return b.value`
        // No semicolons between statements. We need to split by detecting
        // statement boundaries: after assignments (= without chaining) and before 'return'
        let parts: Vec<&str> = if body_str.contains("return") {
            // Split into two parts: before 'return' and from 'return' onwards
            if let Some(pos) = body_str.find("return") {
                let before_return = body_str[..pos].trim();
                let from_return = body_str[pos..].trim();
                let mut p = Vec::new();
                if !before_return.is_empty() { p.push(before_return); }
                p.push(from_return);
                p
            } else {
                vec![body_str.trim()]
            }
        } else {
            vec![body_str.trim()]
        };
        
        for part in &parts {
            let trimmed = part.trim();
            if trimmed.is_empty() { continue; }
            if let Ok(expr) = syn::parse_str::<Expr>(trimmed) {
                stmts.push(expr);
            }
        }

        Ok(ReceiverFn { recv, ident, inputs, output, stmts })
    }
}
