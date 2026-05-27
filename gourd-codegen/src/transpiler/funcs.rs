use super::go_to_rust;
use proc_macro2::TokenStream;
use quote::quote;
use syn::ext::IdentExt;
use syn::parse::discouraged::Speculative;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token;
use syn::{Expr, Ident};
use super::{GoFnInputs, GoFnOutput, map_go_types};

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
                        let renamed = replace_receiver(expr, &recv_name);
                        let transpiled = go_to_rust(&renamed);
                        stmts.push(transpiled);
                    }
                }
            }

            let body: Box<syn::ExprBlock> = syn::parse_quote!({ #(#stmts);* });

            quote! {
                impl #generics #struct_ty {
                    fn #fn_name (#self_arg, #(#all_params),*) #output #body
                }
            }
        }
        Err(e) => e.to_compile_error(),
    }
}

/// Replace all occurrences of `receiver_name` (as a path or field base)
/// with `self` in a Go expression AST. This operates on the RAW Go AST
/// (syn::Expr), producing a new syn::Expr where all receiver references
/// have been renamed to "self". The result can then be passed to `go_to_rust`
/// for full Go→Rust transpilation.
pub(crate) fn replace_receiver(expr: &Expr, recv_name: &Ident) -> Expr {
    match expr {
        Expr::Field(f) => {
            if let Expr::Path(ref base_path) = *f.base
                && base_path.path.is_ident(recv_name) {
                // f.recv_fieldname  →  self.fieldname
                let member = f.member.clone();
                return syn::parse_quote! { self.#member };
            }
            Expr::Field(syn::ExprField {
                attrs: Vec::new(),
                base: Box::new(replace_receiver(&f.base, recv_name)),
                dot_token: f.dot_token,
                member: f.member.clone(),
            })
        }
        Expr::Binary(b) => {
            Expr::Binary(syn::ExprBinary {
                attrs: Vec::new(),
                left: Box::new(replace_receiver(&b.left, recv_name)),
                op: b.op,
                right: Box::new(replace_receiver(&b.right, recv_name)),
            })
        }
        Expr::Unary(u) => {
            Expr::Unary(syn::ExprUnary {
                attrs: Vec::new(),
                op: u.op,
                expr: Box::new(replace_receiver(&u.expr, recv_name)),
            })
        }
        Expr::Call(c) => {
            Expr::Call(syn::ExprCall {
                attrs: Vec::new(),
                func: Box::new(replace_receiver(&c.func, recv_name)),
                paren_token: c.paren_token,
                args: c.args.clone(),
            })
        }
        Expr::MethodCall(mc) => {
            Expr::MethodCall(syn::ExprMethodCall {
                attrs: Vec::new(),
                receiver: Box::new(replace_receiver(&mc.receiver, recv_name)),
                dot_token: mc.dot_token,
                method: mc.method.clone(),
                turbofish: mc.turbofish.clone(),
                paren_token: mc.paren_token,
                args: mc.args.clone(),
            })
        }
        Expr::Index(i) => {
            Expr::Index(syn::ExprIndex {
                attrs: Vec::new(),
                expr: Box::new(replace_receiver(&i.expr, recv_name)),
                bracket_token: i.bracket_token,
                index: Box::new(replace_receiver(&i.index, recv_name)),
            })
        }
        Expr::Array(a) => {
            Expr::Array(syn::ExprArray {
                attrs: Vec::new(),
                bracket_token: a.bracket_token,
                elems: a.elems.iter().map(|e| replace_receiver(e, recv_name)).collect(),
            })
        }
        Expr::Tuple(t) => {
            Expr::Tuple(syn::ExprTuple {
                attrs: Vec::new(),
                paren_token: t.paren_token,
                elems: t.elems.iter().map(|e| replace_receiver(e, recv_name)).collect(),
            })
        }
        Expr::Cast(c) => {
            Expr::Cast(syn::ExprCast {
                attrs: Vec::new(),
                expr: Box::new(replace_receiver(&c.expr, recv_name)),
                as_token: c.as_token,
                ty: c.ty.clone(),
            })
        }
        Expr::Paren(p) => {
            Expr::Paren(syn::ExprParen {
                attrs: Vec::new(),
                expr: Box::new(replace_receiver(&p.expr, recv_name)),
                paren_token: p.paren_token,
            })
        }
        Expr::Group(g) => {
            Expr::Group(syn::ExprGroup {
                attrs: Vec::new(),
                expr: Box::new(replace_receiver(&g.expr, recv_name)),
                group_token: g.group_token,
            })
        }
        Expr::Assign(a) => {
            Expr::Assign(syn::ExprAssign {
                attrs: Vec::new(),
                left: Box::new(replace_receiver(&a.left, recv_name)),
                eq_token: a.eq_token,
                right: Box::new(replace_receiver(&a.right, recv_name)),
            })
        }
        Expr::Path(p) => {
            if p.path.is_ident(recv_name) {
                Expr::Path(syn::ExprPath {
                    attrs: Vec::new(),
                    qself: None,
                    path: syn::Path::from(Ident::new("self", proc_macro2::Span::call_site())),
                })
            } else {
                expr.clone()
            }
        }
        Expr::Lit(_l) => expr.clone(),
        Expr::Range(r) => Expr::Range(syn::ExprRange {
            attrs: Vec::new(),
            start: r.start.as_ref().map(|e| Box::new(replace_receiver(e, recv_name))),
            end: r.end.as_ref().map(|e| Box::new(replace_receiver(e, recv_name))),
            limits: r.limits,
        }),
        Expr::Break(b) => {
            Expr::Break(syn::ExprBreak {
                attrs: Vec::new(),
                break_token: b.break_token,
                label: b.label.clone(),
                expr: b.expr.as_ref().map(|e| Box::new(replace_receiver(e, recv_name))),
            })
        }
        Expr::Return(re) => {
            Expr::Return(syn::ExprReturn {
                attrs: Vec::new(),
                return_token: re.return_token,
                expr: re.expr.as_ref().map(|e| Box::new(replace_receiver(e, recv_name))),
            })
        }
        Expr::If(i) => {
            Expr::If(syn::ExprIf {
                attrs: Vec::new(),
                if_token: i.if_token,
                cond: Box::new(replace_receiver(&i.cond, recv_name)),
                then_branch: i.then_branch.clone(),
                else_branch: i.else_branch.as_ref().map(|(e, block)| (*e, Box::new(replace_receiver(block, recv_name)))),
            })
        }
        Expr::While(w) => {
            Expr::While(syn::ExprWhile {
                attrs: Vec::new(),
                label: w.label.clone(),
                while_token: w.while_token,
                cond: Box::new(replace_receiver(&w.cond, recv_name)),
                body: w.body.clone(),
            })
        }
        Expr::Loop(l) => {
            Expr::Loop(syn::ExprLoop {
                attrs: Vec::new(),
                label: l.label.clone(),
                loop_token: l.loop_token,
                body: l.body.clone(),
            })
        }
        Expr::ForLoop(f) => {
            Expr::ForLoop(syn::ExprForLoop {
                attrs: Vec::new(),
                label: f.label.clone(),
                for_token: f.for_token,
                in_token: f.in_token,
                pat: f.pat.clone(),
                expr: Box::new(replace_receiver(&f.expr, recv_name)),
                body: f.body.clone(),
            })
        }
        Expr::Block(b) => {
            let new_stmts: Vec<syn::Stmt> = b.block.stmts.iter().map(|stm| {
                match stm {
                    syn::Stmt::Expr(v, s) => syn::Stmt::Expr(*Box::new(replace_receiver(v, recv_name)), *s),
                    syn::Stmt::Local(l) => syn::Stmt::Local(replace_receiver_local(l, recv_name)),
                    other => other.clone(),
                }
            }).collect();
            Expr::Block(syn::ExprBlock {
                attrs: Vec::new(),
                label: b.label.clone(),
                block: syn::parse_quote!({ #(#new_stmts);* }),
            })
        }
        Expr::Let(l) => {
            Expr::Let(syn::ExprLet {
                attrs: Vec::new(),
                let_token: l.let_token,
                pat: l.pat.clone(),
                eq_token: l.eq_token,
                expr: Box::new(replace_receiver(&l.expr, recv_name)),
            })
        }
        // All other expression types pass through unchanged
        _ => expr.clone(),
    }
}

/// Replace receiver references in a `syn::Local` (short variable declaration).
pub(crate) fn replace_receiver_local(local: &syn::Local, recv_name: &Ident) -> syn::Local {
    let attrs: Vec<syn::Attribute> = Vec::new();
    let let_token = local.let_token;
    let pat = local.pat.clone();
    let semi_token = local.semi_token;

    let init = match &local.init {
        Some(init) => {
            let expr = replace_receiver(&init.expr, recv_name);
            Some(syn::LocalInit {
                expr: Box::new(expr),
                eq_token: init.eq_token,
                diverge: init.diverge.clone(),
            })
        }
        None => None,
    };

    syn::Local {
        attrs,
        let_token,
        pat,
        init,
        semi_token,
    }
}
