//! Switch statement parsing: `Parse` impl for `Switch`.

use quote::quote;
pub(crate) use super::ast::{GoStmt, Switch, SwitchCase};
use syn::ext::IdentExt;
use syn::parse::{discouraged::Speculative, Parse, ParseStream};
use syn::{Expr, Ident};

impl Parse for Switch {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _switch_kw: Ident = input.call(Ident::parse_any)?;
        eprintln!("DEBUG: switch parser - after switch keyword, checking for selector");

        // Parse optional selector expression (stop at `{` boundary)
        let selector = if input.peek(syn::token::Brace) {
            eprintln!("DEBUG: switch parser - selector is None (no selector)");
            None
        } else {
            eprintln!("DEBUG: switch parser - parsing selector as Path");
            let path: syn::Path = input.parse()?;
            let path_str = quote! { #path }.to_string();
            eprintln!("DEBUG: switch parser - selector path: {}", path_str);
            Some(syn::Expr::Path(syn::ExprPath {
                attrs: Vec::new(),
                qself: None,
                path,
            }))
        };

        let brace_content;
        let _brace = syn::braced!(brace_content in input);
        eprintln!("DEBUG: switch parser - parsed brace content, is_empty={}", brace_content.is_empty());

        let mut cases = Vec::new();
        let mut default_stmts = Vec::new();

        eprintln!("DEBUG: switch parser - starting case parsing loop");
        while !brace_content.is_empty() {
            let fork = brace_content.fork();
            eprintln!("DEBUG: switch parser - checking for case keyword");
            if fork.peek(syn::Ident) {
                if let Ok(kw) = fork.parse::<syn::Ident>() {
                    let kw_str = kw.to_string();
                    eprintln!("DEBUG: switch parser - parsed keyword: {}", kw_str);
                    if kw_str == "case" {
                        brace_content.parse::<syn::Ident>()?;

                        let mut exprs = Vec::new();
                        while !brace_content.peek(syn::token::Colon) && !brace_content.is_empty() {
                            let kw_fork = brace_content.fork();
                            if kw_fork.peek(syn::Ident) {
                                let kw = kw_fork.parse::<syn::Ident>();
                                if let Ok(kw) = kw {
                                    let kw_str = kw.to_string();
                                    if matches!(kw_str.as_str(),
                                        "if" | "for" | "return" | "switch" | "case" | "default") {
                                        break;
                                    }
                                }
                            }
                            let case_fork = brace_content.fork();
                            if case_fork.peek(syn::Lit) {
                                let lit: syn::Lit = case_fork.parse()?;
                                brace_content.advance_to(&case_fork);
                                exprs.push(Expr::Lit(syn::ExprLit {
                                    attrs: Vec::new(),
                                    lit,
                                }));
                            } else if case_fork.peek(syn::Ident) {
                                let path: syn::Path = case_fork.parse()?;
                                brace_content.advance_to(&case_fork);
                                exprs.push(Expr::Path(syn::ExprPath {
                                    attrs: Vec::new(),
                                    qself: None,
                                    path,
                                }));
                            } else {
                                if brace_content.peek(syn::token::Comma) {
                                    let _: syn::token::Comma = brace_content.parse()?;
                                } else {
                                    brace_content.parse::<proc_macro2::TokenTree>()?;
                                }
                            }
                        }
                        let _: syn::token::Colon = brace_content.parse()?;

                        let mut body_stmts = Vec::new();
                        while !brace_content.is_empty() && !brace_content.peek(syn::Ident) {
                            let stmt_fork = brace_content.fork();
                            if let Ok(expr) = stmt_fork.parse::<Expr>() {
                                brace_content.advance_to(&stmt_fork);
                                body_stmts.push(GoStmt::Expr(expr));
                            } else {
                                break;
                            }
                        }

                        cases.push(SwitchCase { exprs, stmts: body_stmts });
                        continue;
                    } else if kw_str == "default" {
                        brace_content.parse::<syn::Ident>()?;
                        let _: syn::token::Colon = brace_content.parse()?;

                        let mut body_stmts = Vec::new();
                        while !brace_content.is_empty() && !brace_content.peek(syn::Ident) {
                            let stmt_fork = brace_content.fork();
                            if let Ok(expr) = stmt_fork.parse::<Expr>() {
                                brace_content.advance_to(&stmt_fork);
                                body_stmts.push(GoStmt::Expr(expr));
                            } else {
                                break;
                            }
                        }

                        default_stmts = body_stmts;
                        continue;
                    }
                }
            }
            break;
        }

        Ok(Switch { selector, cases, default_stmts })
    }
}
