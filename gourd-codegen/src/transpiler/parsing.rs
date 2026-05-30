//! Go source parsing: structs and `Parse` impls for function, block,
//! and parameter declarations.

use proc_macro2::TokenStream;
use quote::quote;
use syn::ext::IdentExt;
use syn::parse::{discouraged::Speculative, Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token;
use syn::{Expr, Ident, Stmt};

use super::expr::go_to_rust;

// ─── Go source AST types ───────────────────────────────────────────────

pub(crate) enum GoStmt {
    Local(syn::Local),
    Expr(Expr),
    GoSlice(Vec<Expr>),
    GoMap(String, Option<syn::Type>, Option<syn::Type>, Vec<(Expr, Expr)>), // (ident, key_type, val_type, entries)
    Switch(Switch),
}

pub(crate) struct GoBlock {
    pub stmts: Vec<GoStmt>,
}

pub(crate) struct GoFnInputs {
    pub args: Vec<GoParam>,
}

pub(crate) struct GoParam {
    pub id: Ident,
    pub ty: Option<Box<syn::Type>>,
    pub slice_elem: Option<syn::Type>,
}

pub(crate) struct GoFnOutput {
    pub tys: Vec<syn::Type>,
}

pub(crate) struct GoFn {
    pub(crate) ident: Ident,
    pub(crate) generics: Punctuated<syn::GenericParam, token::Comma>,
    pub(crate) inputs: GoFnInputs,
    pub(crate) output: Option<GoFnOutput>,
    pub(crate) block: GoBlock,
}

pub(crate) struct GoStruct {
    pub(crate) ident: Ident,
    pub(crate) fields: Vec<GoStructField>,
}

pub(crate) struct GoStructField {
    pub(crate) name: Ident,
    pub(crate) ty: syn::Type,
}

// ─── Switch parsing ────────────────────────────────────────────────────

pub(crate) struct Switch {
    pub(crate) selector: Option<Expr>,
    pub(crate) cases: Vec<SwitchCase>,
    pub(crate) default_stmts: Vec<GoStmt>,
}

pub(crate) struct SwitchCase {
    pub(crate) exprs: Vec<Expr>,
    pub(crate) stmts: Vec<GoStmt>,
}

impl Parse for Switch {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _switch_kw: Ident = input.call(Ident::parse_any)?;

        // Parse optional selector expression (stop at `{` boundary)
        let selector = if input.peek(syn::token::Brace) {
            None
        } else {
            // Parse just a Path to avoid `x { }` being consumed as verbatim
            let path: syn::Path = input.parse()?;
            Some(syn::Expr::Path(syn::ExprPath {
                attrs: Vec::new(),
                qself: None,
                path,
            }))
        };

        let brace_content;
        let _brace = syn::braced!(brace_content in input);

        let mut cases = Vec::new();
        let mut default_stmts = Vec::new();

        while !brace_content.is_empty() {
            // Check for case keyword
            let fork = brace_content.fork();
            if fork.peek(syn::Ident) {
                if let Ok(kw) = fork.parse::<syn::Ident>() {
                    let kw_str = kw.to_string();
                    if kw_str == "case" {
                        // Parse case block
                        brace_content.parse::<syn::Ident>()?;

                        let mut exprs = Vec::new();
                        // Parse comma-separated expressions until `:`
                        loop {
                            if brace_content.peek(syn::token::Colon) {
                                break;
                            }
                            let expr: Expr = brace_content.parse()?;
                            exprs.push(expr);
                            if brace_content.peek(syn::token::Comma) {
                                let _: syn::token::Comma = brace_content.parse()?;
                            } else {
                                break;
                            }
                        }

                        // Consume the colon
                        let _: syn::token::Colon = brace_content.parse()?;

                        // Parse body statements
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
                        // Parse default block
                        brace_content.parse::<syn::Ident>()?;
                        let _: syn::token::Colon = brace_content.parse()?;

                        // Parse body statements
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

// In Go, switch is a statement within a function body.
// We represent it as a statement variant.


// ─── Go parameter parsing (supports shorthand grouping) ────────────────

impl Parse for GoFnInputs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = Vec::new();
        while !input.is_empty() {
            let id: Ident = input.parse()?;
            let mut group_ids: Vec<Ident> = Vec::new();

            // Look ahead for grouped parameters: `a, b, c int`
            while input.peek(token::Comma) {
                let peek_fork = input.fork();
                let _ = peek_fork.parse::<token::Comma>();
                if peek_fork.peek(Ident) {
                    let name = peek_fork.parse::<Ident>()?;
                    let name_str = name.to_string();
                    let known_go_type = matches!(name_str.as_str(),
                        "bool" | "string" | "int" | "int8" | "int16" | "int32" | "int64"
                        | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr"
                        | "byte" | "rune" | "float32" | "float64" | "error"
                    );
                    if known_go_type {
                        input.advance_to(&peek_fork);
                        break;
                    }
                    input.parse::<token::Comma>()?;
                    let param_name: Ident = input.parse()?;
                    group_ids.push(param_name);
                } else {
                    break;
                }
            }

            let fork = input.fork();
            let is_slice_like = fork.peek(syn::token::Bracket);

            let mut ty_from_ident: Option<Box<syn::Type>> = None;
            if !is_slice_like && fork.peek(syn::Ident) {
                ty_from_ident = Some(input.parse()?);
            } else if !is_slice_like && fork.peek(syn::token::Colon) {
                let _colon: syn::token::Colon = input.parse()?;
                ty_from_ident = Some(input.parse()?);
            }

            if is_slice_like {
                let content;
                let _ = syn::bracketed!(content in input);
                let elem_path: syn::Path = if content.is_empty() {
                    input.parse()?
                } else {
                    content.parse()?
                };
                let elem_type = syn::Type::Path(syn::TypePath {
                    path: elem_path,
                    qself: None,
                });
                args.push(GoParam { id: id.clone(), ty: None, slice_elem: Some(elem_type.clone()) });
                for param_id in group_ids {
                    args.push(GoParam { id: param_id, ty: None, slice_elem: Some(elem_type.clone()) });
                }
            } else {
                args.push(GoParam { id: id.clone(), ty: ty_from_ident.clone(), slice_elem: None });
                for param_id in group_ids {
                    args.push(GoParam { id: param_id, ty: ty_from_ident.clone(), slice_elem: None });
                }
            }

            if input.peek(token::Comma) {
                input.parse::<token::Comma>()?;
            }
        }
        Ok(GoFnInputs { args })
    }
}

impl Parse for GoFnOutput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut tys = Vec::new();
        if input.peek(syn::token::RArrow) {
            let _: syn::token::RArrow = input.parse()?;
        }
        if !input.peek(syn::token::Brace) {
            let t = input.parse()?;
            tys.push(t);
            while input.peek(token::Comma) {
                let _ = input.parse::<token::Comma>()?;
                if input.peek(syn::token::Brace) {
                    break;
                }
                let t = input.parse()?;
                tys.push(t);
            }
        }
        Ok(GoFnOutput { tys })
    }
}

impl Parse for GoFn {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _fn: Ident = input.call(Ident::parse_any)?;
        let ident: Ident = input.parse()?;
        let generics = Punctuated::<syn::GenericParam, token::Comma>::new();
        if input.peek(syn::token::Bracket) {
            let content;
            let _bracketed = syn::bracketed!(content in input);
            Punctuated::<syn::GenericParam, token::Comma>::parse_terminated(&content)?;
        }
        let paren_content;
        let _paren = syn::parenthesized!(paren_content in input);
        let inputs = paren_content.parse()?;
        let output = if !input.is_empty() {
            let outer = input.parse()?;
            Some(outer)
        } else {
            None
        };
        let block = parse_go_block(input)?;
        Ok(GoFn { ident, generics, inputs, output, block })
    }
}

impl Parse for GoStruct {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _struct: Ident = input.call(Ident::parse_any)?;
        let ident: Ident = input.parse()?;
        let content;
        let _brace = syn::braced!(content in input);
        let mut fields = Vec::new();
        while !content.is_empty() {
            let name: Ident = content.parse()?;
            let ty: syn::Type = content.parse()?;
            fields.push(GoStructField { name, ty });
            if !content.is_empty() {
                let _comma: token::Comma = content.parse()?;
            }
        }
        Ok(GoStruct { ident, fields })
    }
}

// ─── Statement block parsing ───────────────────────────────────────────

pub(crate) fn parse_go_block(input: ParseStream) -> syn::Result<GoBlock> {
    let brace_content;
    let _brace = syn::braced!(brace_content in input);

    let mut stmts = Vec::new();
    while !brace_content.is_empty() {
        // 1. Try syn::Local (handles `let x = ...` declarations)
        let fork = brace_content.fork();
        if fork.peek(syn::token::Let) {
            match fork.parse::<Stmt>() {
                Ok(Stmt::Local(_)) => {
                    if let Ok(Stmt::Local(local)) = brace_content.parse() {
                        stmts.push(GoStmt::Local(local));
                        if brace_content.peek(token::Semi) {
                            let _semi: token::Semi = brace_content.parse()?;
                        }
                        continue;
                    }
                }
                _ => {}
            }
            // Handle `let m = map[K]V{...}` when syn can't parse the value
            let let_fork = brace_content.fork();
            if let_fork.parse::<syn::token::Let>().is_ok()
                && let_fork.parse::<syn::Ident>().is_ok()
                && let_fork.parse::<syn::token::Eq>().is_ok()
            {
                if let_fork.peek(syn::Ident) {
                    let map_fork = let_fork.fork();
                    if let Ok(kw) = map_fork.parse::<syn::Ident>() {
                        if kw == "map" && map_fork.peek(syn::token::Bracket) {
                            // Parse: `let m = map[K]V{entries}`
                            brace_content.parse::<syn::token::Let>()?;
                            let ident = brace_content.parse::<syn::Ident>()?;
                            let ident_str = ident.to_string();
                            brace_content.parse::<syn::token::Eq>()?;
                            // Parse map literal `map[K]V{entries}`
                            brace_content.parse::<syn::Ident>()?;
                            // Capture key type
                            let k_content;
                            let _ = syn::bracketed!(k_content in brace_content);
                            let key_type = if !k_content.is_empty() {
                                if let Ok(t) = k_content.parse::<syn::Type>() {
                                    Some(t)
                                } else {
                                    let _: TokenStream = k_content.parse()?;
                                    None
                                }
                            } else {
                                None
                            };
                            // Capture value type
                            let val_type = if brace_content.peek(syn::Ident) || brace_content.peek(syn::token::Bracket) {
                                if let Ok(t) = brace_content.parse::<syn::Type>() {
                                    Some(t)
                                } else {
                                    let _: TokenStream = brace_content.parse()?;
                                    None
                                }
                            } else {
                                None
                            };
                            // Skip `{...}` group and parse entries
                            let m_content = brace_content.step(|cursor| {
                                if let Some((inner, _, rest)) = cursor.group(proc_macro2::Delimiter::Brace) {
                                    Ok((inner.token_stream(), rest))
                                } else {
                                    Err(cursor.error("expected `{`"))
                                }
                            });
                            let mut entries = Vec::new();
                            if let Ok(inner_ts) = m_content {
                                if !inner_ts.is_empty() {
                                    let parser: MapEntryParser = syn::parse2(inner_ts).unwrap_or_default();
                                    entries = parser.entries;
                                }
                                stmts.push(GoStmt::GoMap(ident_str, key_type, val_type, entries));
                                if brace_content.peek(token::Semi) {
                                    let _semi: token::Semi = brace_content.parse()?;
                                }
                                continue;
                            }
                        }
                    }
                }
            }
        }

        // 2. Check for Go slice literal: []...{...}
        let fork = brace_content.fork();
        if fork.peek(syn::token::Bracket) {
            let _bracket;
            let bracket_content;
            _bracket = syn::bracketed!(bracket_content in fork);

            if !bracket_content.is_empty() {
                let type_fork = bracket_content.fork();
                if type_fork.parse::<syn::Type>().is_ok() {
                    let _: syn::Type = bracket_content.parse()?;
                } else {
                    let _: TokenStream = bracket_content.parse()?;
                }
            }

            let type_fork = fork.fork();
            if type_fork.parse::<syn::Type>().is_ok() {
                let _: syn::Type = fork.parse()?;
            }

            if fork.peek(syn::token::Brace) {
                use proc_macro2::TokenTree;
                brace_content.advance_to(&fork);
                let group_stream: TokenStream = fork.parse()?;
                if let Some(TokenTree::Group(group)) = group_stream
                    .into_iter()
                    .next()
                    .and_then(|t| {
                        if let TokenTree::Group(g) = t { Some(TokenTree::Group(g)) } else { None }
                    })
                    && group.delimiter() == proc_macro2::Delimiter::Brace
                {
                    let inner_ts = group.stream();
                    let mut elems = Vec::new();
                    if !inner_ts.is_empty() {
                        let parser: ElemParser = syn::parse2(inner_ts).unwrap_or_default();
                        elems = parser.elems;
                    }
                    let _rest = brace_content.step(|cursor| {
                        if let Some((_, _, rest)) = cursor.group(proc_macro2::Delimiter::Brace) {
                            Ok(((), rest))
                        } else {
                            Err(cursor.error("expected `{`"))
                        }
                    });
                    stmts.push(GoStmt::GoSlice(elems));
                    continue;
                }
            }
        }

        // 3. Check for switch statement
        let fork = brace_content.fork();
        if fork.peek(syn::Ident) {
            if let Ok(kw) = fork.parse::<syn::Ident>() {
                let kw_str = kw.to_string();
                if kw_str == "switch" {
                    let parsed_switch = brace_content.parse::<Switch>()?;
                    stmts.push(GoStmt::Switch(parsed_switch));
                    continue;
                }
            }
        }

        // 4. Try standard expression parsing via speculative parse
        let fork = brace_content.fork();
        if let Ok(expr) = fork.parse::<Expr>() {
            brace_content.advance_to(&fork);
            stmts.push(GoStmt::Expr(expr));
            if brace_content.peek(token::Semi) {
                let _semi: token::Semi = brace_content.parse()?;
            }
            continue;
        }

        // 4. Check for Go map literal: map[K]V{...}
        let fork = brace_content.fork();
        if fork.peek(syn::Ident) {
            if let Ok(ident) = fork.parse::<syn::Ident>() {
                if ident == "map" && fork.peek(syn::token::Bracket) {
                    brace_content.parse::<syn::Ident>()?;
                    let k_content;
                    let _ = syn::bracketed!(k_content in brace_content);
                    if !k_content.is_empty() {
                        if k_content.parse::<syn::Type>().is_err() {
                            let _: TokenStream = k_content.parse()?;
                        }
                    }
                    if brace_content.peek(syn::Ident) || brace_content.peek(syn::token::Bracket) {
                        if brace_content.parse::<syn::Type>().is_err() {
                            let _: TokenStream = brace_content.parse()?;
                        }
                    }
                    let m_content = brace_content.step(|cursor| {
                        if let Some((inner, _, rest)) = cursor.group(proc_macro2::Delimiter::Brace) {
                            Ok((inner.token_stream(), rest))
                        } else {
                            Err(cursor.error("expected `{`"))
                        }
                    });
                    let mut entries = Vec::new();
                    if let Ok(inner_ts) = m_content {
                        if !inner_ts.is_empty() {
                            let parser: MapEntryParser = syn::parse2(inner_ts).unwrap_or_default();
                            entries = parser.entries;
                        }
                        stmts.push(GoStmt::GoMap(String::new(), None, None, entries));
                        continue;
                    }
                }
            }
        }

        // 5. Nothing matched — skip one token to avoid infinite loop
        let _ = brace_content.parse::<syn::Expr>();
    }

    Ok(GoBlock { stmts })
}

// ─── Inline parse helpers used inside parse_go_block and free_fn ───────

#[derive(Default)]
struct ElemParser {
    elems: Vec<Expr>,
}
impl Parse for ElemParser {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut elems = Vec::new();
        while !input.is_empty() {
            let expr: Expr = input.parse()?;
            elems.push(expr);
            if input.peek(syn::token::Comma) {
                let _: syn::token::Comma = input.parse()?;
            } else {
                break;
            }
        }
        Ok(ElemParser { elems })
    }
}

#[derive(Default)]
struct MapEntryParser {
    entries: Vec<(Expr, Expr)>,
}
impl Parse for MapEntryParser {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut entries = Vec::new();
        while !input.is_empty() {
            if let Ok(key) = input.parse::<Expr>() {
                if input.peek(syn::token::Colon) {
                    let _: token::Colon = input.parse()?;
                    if let Ok(value) = input.parse::<Expr>() {
                        entries.push((key, value));
                        if input.peek(token::Comma) {
                            let _: token::Comma = input.parse()?;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        Ok(MapEntryParser { entries })
    }
}

// ─── Free function: go_stmt_to_rust (bridging function) ────────────────

pub(crate) fn go_stmt_to_rust(stmt: &GoStmt) -> TokenStream {
    match stmt {
        GoStmt::Local(local) => {
            let pat = &local.pat;
            let val = local.init.as_ref().map(|v| go_to_rust(&v.expr));
            quote! { let #pat = #val; }
        }
        GoStmt::Expr(expr) => go_to_rust(expr),
        GoStmt::GoSlice(elems) => {
            let elems: Vec<_> = elems.iter().map(go_to_rust).collect();
            quote! { vec![ #(#elems),* ] }
        }
        GoStmt::GoMap(ident, key_type, val_type, entries) => {
            go_stmt_to_rust_map(ident, key_type, val_type, entries)
        }
        GoStmt::Switch(switch) => {
            super::free_fn::transpile_switch(switch)
        }
    }
}

fn go_stmt_to_rust_map(
    ident: &str,
    key_type: &Option<syn::Type>,
    val_type: &Option<syn::Type>,
    entries: &[(Expr, Expr)],
) -> TokenStream {
    use super::types::map_go_types;

    if entries.is_empty() {
        if ident.is_empty() {
            return quote! { std::collections::HashMap::default() };
        }
        let name: syn::Ident = syn::parse_str(ident).unwrap();
        if let (Some(kt), Some(vt)) = (key_type, val_type) {
            let kt = map_go_types(kt);
            let vt = map_go_types(vt);
            return quote! { let #name = std::collections::HashMap::<#kt, #vt>::default(); };
        }
        return quote! { let #name = std::collections::HashMap::default(); };
    }

    let insertions: Vec<_> = entries.iter().map(|(k, v)| {
        let key = go_to_rust(k);
        let val = go_to_rust(v);
        quote! { m.insert(#key, #val); }
    }).collect();

    let block = if let (Some(kt), Some(vt)) = (key_type, val_type) {
        let kt = map_go_types(kt);
        let vt = map_go_types(vt);
        quote! {
            {
                let mut m = std::collections::HashMap::<#kt, #vt>::new();
                #(#insertions)*
                m
            }
        }
    } else {
        quote! {
            {
                let mut m = std::collections::HashMap::new();
                #(#insertions)*
                m
            }
        }
    };

    if ident.is_empty() {
        block
    } else {
        let name: syn::Ident = syn::parse_str(ident).unwrap();
        quote! { let #name = #block; }
    }
}
