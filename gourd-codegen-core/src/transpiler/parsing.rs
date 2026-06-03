//! Go source parsing: structs and `Parse` impls for function, block,
//! and parameter declarations.

use proc_macro2::TokenStream;
use quote::quote;
use syn::ext::IdentExt;
use syn::parse::{discouraged::Speculative, Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token;
use syn::{parse_quote, Expr, Ident, Stmt, Token};

use super::expr::{dispatch, go_to_rust};

// ─── Go source AST types ───────────────────────────────────────────────

pub(crate) enum GoStmt {
    Local(syn::Local),
    GoLocal(Ident, TokenStream),  // Go short variable declaration: `id := expr`
    If(GoIf),
    Expr(Expr),
    GoSlice(Vec<Expr>),
    GoMap(String, Option<Box<syn::Type>>, Option<Box<syn::Type>>, Vec<(Expr, Expr)>), // (ident, key_type, val_type, entries)
    GoReturn(Vec<Expr>),  // multi-return: `return a, b`
    Switch(Switch),
    Continue,
    While(GoWhile),
    GoFor(GoFor),
    GoChannelSend(Expr, Expr), // `ch <- value`
    GoChannelRecv(Expr),       // `<- ch`
    GoTypeAssert(Expr, syn::Type), // `x.(T)` type assertion
}

pub(crate) struct GoFor {
    /// Optional init (e.g., `i := 0` or `i, v := `)
    pub init: Option<GoForInit>,
    /// Always true for `for` with `range`
    pub is_range: bool,
    /// The iterable expression (parsed as Path to avoid syn eating braces)
    pub iterable: syn::Path,
    /// The loop body
    pub body: GoBlock,
}

pub(crate) enum GoForInit {
    /// Single variable: `for i := range slice`
    Single(Ident),
    /// Two variables: `for i, v := range slice`
    Double(Ident, Ident),
}

pub(crate) struct GoWhile {
    pub cond: Expr,
    pub body: GoBlock,
}

pub(crate) struct GoIf {
    pub cond: Expr,
    pub then_block: GoBlock,
    pub else_block: Option<GoBlock>,
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
    pub is_slice: bool,
    pub elem_type: Option<Box<syn::Type>>, // element type for slice returns
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

// ─── Interface parsing ─────────────────────────────────────────────────

pub(crate) struct GoInterface {
    pub(crate) ident: Ident,
    pub(crate) methods: Vec<GoInterfaceMethod>,
}

pub(crate) struct GoInterfaceMethod {
    pub(crate) name: Ident,
    pub(crate) inputs: GoFnInputs,
    pub(crate) output: Option<GoFnOutput>,
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

                        // Parse case expressions (supports multi: `case 1, 2, 3:`)
                        // Use fork-to-colon loop - stop at `:` or empty case
                        let mut exprs = Vec::new();
                        while !brace_content.peek(syn::token::Colon) && !brace_content.is_empty() {
                            // Check for Go keywords that start new statements
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
                            // Parse literal or path
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
                                // Stop at comma - handle multi-expression
                                if brace_content.peek(syn::token::Comma) {
                                    let _: syn::token::Comma = brace_content.parse()?;
                                } else {
                                    brace_content.parse::<proc_macro2::TokenTree>()?;
                                }
                            }
                        }
                        // Consume the case colon
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
                        | "byte" | "rune" | "float32" | "float64" | "error" | "chan"
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
                // Special handling for `chan` type - convert `chan int` to `chan<int>`
                let ty = if let Some(ty) = ty_from_ident.clone() {
                    if let syn::Type::Path(tp) = &*ty
                        && tp.path.segments.len() == 1
                        && tp.path.segments.first().unwrap().ident.to_string() == "chan"
                    {
                        // Parse element type and build `chan<T>`
                        if input.peek(syn::Ident) {
                            let elem_ty: syn::Type = input.parse()?;
                            let mut chan_path = syn::Path::from(syn::Ident::new("chan", proc_macro2::Span::call_site()));
                            chan_path.segments.clear();
                            chan_path.segments.push(syn::PathSegment {
                                ident: syn::Ident::new("chan", proc_macro2::Span::call_site()),
                                arguments: syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                                    colon2_token: Default::default(),
                                    lt_token: Token![<](proc_macro2::Span::call_site()),
                                    args: syn::punctuated::Punctuated::from_iter([syn::GenericArgument::Type(elem_ty)]),
                                    gt_token: Token![>](proc_macro2::Span::call_site()),
                                }),
                            });
                            Some(Box::new(syn::Type::Path(syn::TypePath { path: chan_path, qself: None })))
                        } else {
                            Some(ty)
                        }
                    } else {
                        Some(ty)
                    }
                } else { None };
                let ty_for_param = ty.clone();
                args.push(GoParam { id: id.clone(), ty: ty_for_param, slice_elem: None });
                for param_id in group_ids {
                    args.push(GoParam { id: param_id, ty: ty.clone(), slice_elem: None });
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
        let mut is_slice = false;
        let mut elem_type: Option<Box<syn::Type>> = None;
        if input.peek(syn::token::RArrow) {
            let _: syn::token::RArrow = input.parse()?;
        }
        if !input.peek(syn::token::Brace) {
            // Check for Go slice type `[]...` first (before syn's Type parser)
            if input.peek(syn::token::Bracket) {
                is_slice = true;
                let content;
                let _bracket = syn::bracketed!(content in input);
                // Check if next token is `{` - this means we're done with return type
                if input.peek(syn::token::Brace) {
                    // Return type is just `[]` followed by body - push marker and exit
                    tys.push(syn::Type::Path(syn::TypePath {
                        path: syn::Path::from(syn::Ident::new("__go_slice__", proc_macro2::Span::call_site())),
                        qself: None,
                    }));
                } else {
                    // Has type after brackets: `[]int` - STORE the element type
                    let elem = input.parse::<syn::Type>()?;
                    elem_type = Some(Box::new(elem));
                    tys.push(syn::Type::Path(syn::TypePath {
                        path: syn::Path::from(syn::Ident::new("__go_slice__", proc_macro2::Span::call_site())),
                        qself: None,
                    }));
                }
            } else {
                // Standard type parsing
                let t = input.parse()?;
                tys.push(t);
            }
            while input.peek(token::Comma) {
                let _ = input.parse::<token::Comma>()?;
                if input.peek(syn::token::Brace) {
                    break;
                }
                if input.peek(syn::token::Bracket) {
                    is_slice = true;
                    let content;
                    let _bracket = syn::bracketed!(content in input);
                    // Check if next token is `{` - means return type ends here
                    if input.peek(syn::token::Brace) {
                        tys.push(syn::Type::Path(syn::TypePath {
                            path: syn::Path::from(syn::Ident::new("__go_slice__", proc_macro2::Span::call_site())),
                            qself: None,
                        }));
                    } else {
                        let elem = input.parse::<syn::Type>()?;
                        elem_type = Some(Box::new(elem));
                        tys.push(syn::Type::Path(syn::TypePath {
                            path: syn::Path::from(syn::Ident::new("__go_slice__", proc_macro2::Span::call_site())),
                            qself: None,
                        }));
                    }
                } else {
                    let t = input.parse()?;
                    tys.push(t);
                }
            }
        }
        Ok(GoFnOutput { tys, is_slice, elem_type })
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
        // Go struct fields: `name type` (space-separated, no commas).
        while !content.is_empty() {
            let name: Ident = content.parse()?;
            let ty: syn::Type = content.parse()?;
            fields.push(GoStructField { name, ty });
            // Skip whitespace/newlines between fields. Use fork to peek first.
            loop {
                let f = content.fork();
                match f.parse::<proc_macro2::TokenTree>() {
                    Ok(proc_macro2::TokenTree::Punct(p)) if p.as_char() == ',' => {
                        let _comma: token::Comma = content.parse()?;
                        break;  // Found comma - done
                    }
                    // If it's whitespace/newline, consume it and continue
                    Ok(proc_macro2::TokenTree::Punct(_)) => {
                        content.parse::<proc_macro2::TokenTree>()?;
                    }
                    // Non-whitespace token (next field name) - stop skipping
                    Ok(_) => break,
                    Err(_) => break,
                }
            }
        }
        Ok(GoStruct { ident, fields })
    }
}

impl Parse for GoInterface {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _interface_kw: Ident = input.call(Ident::parse_any)?; // consume 'interface'
        let ident: Ident = input.parse()?;
        let content;
        let _brace = syn::braced!(content in input);

        // Parse methods: `Name() type` or `Name(params) type`
        let mut methods = Vec::new();
        while !content.is_empty() {
            let method_fork = content.fork();
            if let Ok(name) = method_fork.parse::<Ident>() {
                let name_str = name.to_string();
                // Skip if it looks like a type keyword (Go type names used as field names)
                if matches!(name_str.as_str(),
                    "bool" | "string" | "int" | "int8" | "int16" | "int32" | "int64"
                    | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr"
                    | "byte" | "rune" | "float32" | "float64" | "error")
                {
                    break; // This is a struct field, not a method name
                }
                // Method names can't be Rust keywords either
                if matches!(name_str.as_str(),
                    "if" | "else" | "for" | "return" | "switch" | "case" | "default"
                    | "type" | "struct" | "func" | "interface" | "package" | "import" | "const" | "var")
                {
                    break;
                }

                // Consume the method name
                content.parse::<Ident>()?;

                // Parse parameters
                let param_paren;
                let _paren = syn::parenthesized!(param_paren in content);
                let inputs: GoFnInputs = param_paren.parse()?;

                // Parse optional return type
                let output = if !content.is_empty() {
                    let out_fork = content.fork();
                    if out_fork.peek(syn::token::RArrow) || out_fork.peek(Ident) || out_fork.peek(syn::token::Bracket) {
                        Some(content.parse::<GoFnOutput>()?)
                    } else {
                        None
                    }
                } else {
                    None
                };

                methods.push(GoInterfaceMethod { name, inputs, output });
            } else {
                break;
            }
        }

        Ok(GoInterface { ident, methods })
    }
}

// ─── Statement block parsing ───────────────────────────────────────────

pub(crate) fn parse_go_block(input: ParseStream) -> syn::Result<GoBlock> {
    let brace_content;
    let _brace = syn::braced!(brace_content in input);

    let mut stmts = Vec::new();
    while !brace_content.is_empty() {
        // Check for Go-specific constructs first (maps, slices, if, switch, return)
        if parse_go_special_stmt(&brace_content, &mut stmts)? {
            continue; // Handled by the special case
        }
        // Fall back to the base parser for common statements
        parse_base_stmt(&brace_content, &mut stmts)?;
    }

    Ok(GoBlock { stmts })
}

/// Try to parse a Go-specific statement from the input.
/// Returns `true` if a statement was parsed (consuming input), `false` to fall back.
pub(crate) fn parse_go_special_stmt(input: ParseStream, stmts: &mut Vec<GoStmt>) -> syn::Result<bool> {
    // 1. Check for Go slice literal: []...{...}
    if input.peek(syn::token::Bracket) {
        if let Ok(()) = parse_go_slice_literal(input, stmts) {
            return Ok(true);
        }
    }

    // 3. Check for if statement (if is a Rust keyword, but in some contexts tokenized as ident)
    if input.peek(syn::token::If) {
        return parse_go_if(input, stmts);
    }
    if input.peek(syn::Ident) {
        let fork = input.fork();
        match fork.parse::<syn::Ident>() {
            Ok(kw) => {
                let kw_str = kw.to_string();
                if kw_str == "if" {
                    return parse_go_if(input, stmts);
                }
            }
            Err(_) => {}
        }
    }

    // 4. Check for switch statement
    if input.peek(syn::Ident) {
        if let Ok(kw) = input.fork().parse::<syn::Ident>() {
            if kw.to_string() == "switch" {
                let parsed_switch = input.parse::<Switch>()?;
                stmts.push(GoStmt::Switch(parsed_switch));
                return Ok(true);
            }
        }
    }

    // 5. Check for return (including multi-return and slice returns)
    if input.peek(syn::token::Return) {
        return parse_go_return(input, stmts);
    }

    // 6. Check for continue statement (continue is a Rust keyword)
    // In proc-macro context, `continue` is tokenized as TokenTree::Ident,
    // so we use fork + TokenTree parsing to detect it
    let cont_fork = input.fork();
    if let Ok(token) = cont_fork.parse::<proc_macro2::TokenTree>() {
        if let proc_macro2::TokenTree::Ident(ident) = token {
            if ident.to_string() == "continue" {
                // consume the continue ident token
                let _token: proc_macro2::TokenTree = input.parse()?;
                stmts.push(GoStmt::Continue);
                if input.peek(token::Semi) {
                    let _semi: token::Semi = input.parse()?;
                }
                return Ok(true);
            }
        }
    }

    // 7. Check for while loop: `while cond { body }`
    // Note: `while` is a Rust keyword, so we use `Token![while]` not `Ident`
    if input.peek(syn::token::While) {
        let parsed_while = parse_go_while(input)?;
        stmts.push(GoStmt::While(parsed_while));
        return Ok(true);
    }

    // 8. Check for for range loop: `for init := range iter` or `for init, v := range iter`
    // Note: `for` is a Rust keyword, so we use `Token![for]` not `Ident`
    if input.peek(syn::token::For) {
        let parsed_for = parse_go_for(input)?;
        stmts.push(GoStmt::GoFor(parsed_for));
        return Ok(true);
    }

    // 9. Check for channel send: `ch <- value`
    if input.peek(syn::Ident) {
        let fork = input.fork();
        let valid = fork.parse::<Ident>().is_ok()
            && fork.cursor().punct()
                .map(|(p, _)| p.as_char() == '<' && p.spacing() == proc_macro2::Spacing::Joint)
                .unwrap_or(false);
        if valid {
            let ch_ident: Ident = input.parse()?;
            let _p1: proc_macro2::Punct = input.parse()?;
            let _p2: proc_macro2::Punct = input.parse()?;
            let val_expr: Expr = input.parse()?;
            stmts.push(GoStmt::GoChannelSend(
                Expr::Path(syn::ExprPath { attrs: vec![], qself: None, path: syn::Path::from(ch_ident) }),
                val_expr,
            ));
            if input.peek(token::Semi) {
                let _semi: token::Semi = input.parse()?;
            }
            return Ok(true);
        }
    }

    // No Go-specific statement matched
    Ok(false)
}

fn parse_go_while(input: ParseStream) -> syn::Result<GoWhile> {
    // Consume 'while' keyword (it's a keyword, not an identifier)
    let _: syn::token::While = input.parse()?;

    // Parse condition
    let cond = input.parse::<Expr>()?;

    // Parse body block
    let body_content;
    let _brace = syn::braced!(body_content in input);

    // Parse body statements
    let mut body_stmts = Vec::new();
    while !body_content.is_empty() {
        if parse_go_special_stmt(&body_content, &mut body_stmts)? {
            continue;
        }
        // Fall back to base parser
        parse_base_stmt(&body_content, &mut body_stmts)?;
    }

    Ok(GoWhile {
        cond,
        body: GoBlock { stmts: body_stmts },
    })
}

/// Parse Go `for` with `range`: `for init := range iter { ... }` or `for init, v := range iter { ... }`
fn parse_go_for(input: ParseStream) -> syn::Result<GoFor> {
    // Consume 'for' keyword (it's a keyword, not an identifier)
    let _: syn::token::For = input.parse()?;
    let peek_ident = input.peek(syn::Ident);
    let _next_ident_str = if peek_ident {
        input.fork().parse::<syn::Ident>().ok().map(|i| i.to_string()).unwrap_or_default()
    } else {
        "<none>".to_string()
    };

    // Parse init OR check for `range` keyword directly
    let init = if input.peek(syn::Ident) {
        // Check if next ident is `range` (for `for range iter { ... }`)
        let fork = input.fork();
        if let Ok(first_ident) = fork.parse::<syn::Ident>() {
            if first_ident.to_string() == "range" {
                // No init, just `for range iter { body }`
                None
            } else {
                // Has init: `i :=` or `i, v :=`
                input.parse::<syn::Ident>()?; // consume ident

                // Check for second variable: `i, v :=`
                if input.peek(syn::token::Comma) {
                    let _: syn::token::Comma = input.parse()?;
                    let second_ident = input.parse::<syn::Ident>()?;
                    // Consume `:=`
                    let _: syn::token::Colon = input.parse()?;
                    let _: syn::token::Eq = input.parse()?;
                    Some(GoForInit::Double(first_ident, second_ident))
                } else {
                    // Single init: `i :=`
                    let _: syn::token::Colon = input.parse()?;
                    let _: syn::token::Eq = input.parse()?;
                    Some(GoForInit::Single(first_ident))
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    // Consume 'range' keyword (if we didn't already consume it above)
    if !matches!(&init, None) || input.peek(syn::Ident) {
        let fork = input.fork();
        match fork.parse::<syn::Ident>() {
            Ok(range_kw) => {
                if range_kw.to_string() == "range" {
                    let _: syn::Ident = input.parse()?; // consume 'range'
                } else {
                    return Err(syn::Error::new(input.span(), "expected `range` keyword"));
                }
            }
            Err(_) => {
                return Err(syn::Error::new(input.span(), "expected `range` keyword"));
            }
        }
    }
    // Parse iterable as Path (not Expr) - syn's Expr::parse on `x {` consumes
    // the brace as verbatim, swallowing the entire for loop body!
    let iterable: syn::Path = input.parse()?;
    let body_content;
    let _brace = syn::braced!(body_content in input);
    let mut body_stmts = Vec::new();
    while !body_content.is_empty() {
        if parse_go_special_stmt(&body_content, &mut body_stmts)? {
            continue;
        }
        parse_base_stmt(&body_content, &mut body_stmts)?;
    }
    Ok(GoFor {
        init,
        is_range: true,
        iterable,
        body: GoBlock { stmts: body_stmts },
    })
}

/// Handle Go-style `id := map[K]V{entries}` map literal declaration.
/// Called from `parse_base_stmt` when a map short-declaration is detected.
fn parse_go_map_decl(input: ParseStream, ident_str: String, stmts: &mut Vec<GoStmt>) -> syn::Result<()> {
    // Parse map literal: `map[K]V{entries}`
    let _kw: syn::Ident = input.parse()?; // consume 'map'

    // Capture key type from `[K]` using manual token advancement
    // (Go tokenizes [ and ] as separate punctuation, not a bracket group)
    let mut key_type: Option<Box<syn::Type>> = None;
    let bracket_fork = input.fork();
    if bracket_fork.peek(syn::token::Bracket) {
        input.advance_to(&bracket_fork);
        let _ts: proc_macro2::TokenTree = input.parse()?; // skip `[`
        // Parse the key type directly from input
        key_type = input.parse::<syn::Type>().ok().map(Box::new);
        // Skip the `]` punctuation token
        if !input.is_empty() && input.peek(syn::token::Bracket) {
            let _ts: proc_macro2::TokenTree = input.parse()?;
        }
    }

    // Capture value type (only if next token isn't `{`)
    let val_type = if !input.peek(syn::token::Brace) {
        input.parse::<syn::Type>().ok().map(Box::new)
    } else {
        None
    };
    // Parse `{...}` entries
    let m_content = input.step(|cursor| {
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
    }
    stmts.push(GoStmt::GoMap(ident_str, key_type, val_type, entries));
    if input.peek(token::Semi) {
        let _semi: token::Semi = input.parse()?;
    }
    Ok(())
}

/// Parse `[]T{...}` slice literal at the start of a statement.
/// Manually advances past `[` and `]` punctuation, then parses elements from `{...}`.
fn parse_go_slice_literal(input: ParseStream, stmts: &mut Vec<GoStmt>) -> syn::Result<()> {
    let fork = input.fork();
    // Check `[` punctuation (not a bracket group - Go has separate `[` and `]` punctuation)
    if fork.peek(syn::token::Bracket) {
        // Manually advance past `[` and `]` punctuation, consuming any type between them
        input.advance_to(&fork); // advance to `[`
        // Skip the `[` punctuation as a TokenTree
        let _ts: proc_macro2::TokenTree = input.parse()?;

        // Consume tokens until we reach `]` or `{`
        while !input.is_empty() && !input.peek(syn::token::Bracket) && !input.peek(syn::token::Brace) {
            let _ = input.parse::<proc_macro2::TokenTree>()?;
        }
        if !input.is_empty() && input.peek(syn::token::Bracket) {
            // Skip the `]` punctuation as a TokenTree
            let _ts: proc_macro2::TokenTree = input.parse()?;
        }
        // Skip any type tokens between `]` and `{`
        while !input.is_empty() && !input.peek(syn::token::Brace) {
            let _ = input.parse::<proc_macro2::TokenTree>()?;
        }

        // Now we should be at `{` - manually parse elements, consume `}`
        if input.peek(syn::token::Brace) {
            let _ts: proc_macro2::TokenTree = input.parse()?; // consume `{`
            // Parse elements until we hit `}`
            let mut elems = Vec::new();
            while !input.is_empty() && !input.peek(syn::token::Brace) {
                let _expr: Expr = input.parse()?;
                elems.push(_expr);
                if input.peek(syn::token::Comma) {
                    let _ = input.parse::<syn::token::Comma>();
                } else {
                    break;
                }
            }
            if input.peek(syn::token::Brace) {
                let _ts: proc_macro2::TokenTree = input.parse()?; // consume `}`
            }
            stmts.push(GoStmt::GoSlice(elems));
            if input.peek(token::Semi) {
                let _semi: token::Semi = input.parse()?;
            }
            return Ok(());
        }
    }
    // Not a slice literal - return error to signal "no match"
    Err(syn::Error::new(proc_macro2::Span::call_site(), "expected slice literal"))
}

/// Parse `if cond { body } else { ... }`.
fn parse_go_if(input: ParseStream, stmts: &mut Vec<GoStmt>) -> syn::Result<bool> {
    input.parse::<syn::token::If>()?; // consume 'if' (it's a Rust keyword)
    let cond: Expr = input.parse()?;
    let then_block_content;
    let _brace = syn::braced!(then_block_content in input);
    let then_block = parse_block_stmts(&then_block_content)?;

    let else_block = if input.peek(syn::token::Else) {
        input.parse::<syn::token::Else>()?; // consume 'else' (Rust keyword)
        if input.peek(syn::token::Brace) {
            let else_block_content;
            let _brace = syn::braced!(else_block_content in input);
            // Use parse_go_block for else body too
            Some(GoBlock { stmts: parse_block_stmts(&else_block_content)? })
        } else {
            None
        }
    } else {
        None
    };

    stmts.push(GoStmt::If(GoIf {
        cond,
        then_block: GoBlock { stmts: then_block },
        else_block,
    }));
    Ok(true)
}

/// Parse `return` - handles `return val`, `return a, b` (multi-return),
/// and `return []T{...}` (slice in return).
fn parse_go_return(input: ParseStream, stmts: &mut Vec<GoStmt>) -> syn::Result<bool> {
    input.parse::<syn::token::Return>()?;

    // Check for channel receive: `return <-ch`
    if input.cursor().punct().is_some() {
        if let Some((p, _)) = input.cursor().punct() {
            if p.as_char() == '<' && p.spacing() == proc_macro2::Spacing::Joint {
                let _p1: proc_macro2::Punct = input.parse()?;
                let _p2: proc_macro2::Punct = input.parse()?;
                let ch_ident: Ident = input.parse()?;
                let ch_expr = Expr::Path(syn::ExprPath { attrs: vec![], qself: None, path: syn::Path::from(ch_ident) });
                stmts.push(GoStmt::GoChannelRecv(ch_expr));
                if input.peek(token::Semi) {
                    let _semi: token::Semi = input.parse()?;
                }
                return Ok(true);
            }
        }
    }

    // Check for `return []T{...}` (Go slice literal in return)
    let adv_fork = input.fork();
    if adv_fork.peek(syn::token::Bracket) {
        // Manually advance past `[` and `]` punctuation, consuming type tokens
        input.advance_to(&adv_fork); // advance to `[`
        // Skip the `[` punctuation token
        let _ts: proc_macro2::TokenTree = input.parse()?;
        // Skip tokens between `[]` and `{`
        while !input.is_empty() && !input.peek(syn::token::Bracket) && !input.peek(syn::token::Brace) {
            let _ = input.parse::<proc_macro2::TokenTree>()?;
        }
        if !input.is_empty() && input.peek(syn::token::Bracket) {
            // Skip the `]` punctuation token
            let _ts: proc_macro2::TokenTree = input.parse()?;
        }
        // Skip any type tokens between `]` and `{`
        while !input.is_empty() && !input.peek(syn::token::Brace) {
            let _ = input.parse::<proc_macro2::TokenTree>()?;
        }
        if input.peek(syn::token::Brace) {
            // Parse the brace-delimited group (Go `{1, 2, 3}` in the token stream)
            let m_content;
            let _ = syn::braced!(m_content in input);
            let mut elems = Vec::new();
            while !m_content.is_empty() {
                if let Ok(expr) = m_content.parse::<Expr>() {
                    elems.push(expr);
                    if m_content.peek(syn::token::Comma) {
                        let _ = m_content.parse::<syn::token::Comma>();
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            let rust_elems: Vec<_> = elems.iter().map(|e| go_to_rust(e)).collect();
            stmts.push(GoStmt::Expr(parse_quote! { return vec![ #(#rust_elems),* ] }));
            if input.peek(token::Semi) {
                let _semi: token::Semi = input.parse()?;
            }
            return Ok(true);
        }
    }

    // Check for multi-return: `return a, b`
    let after_ret = input.fork();
    if !after_ret.is_empty() {
        // Check for Go type assertion: `return x.(T)` or chained `x.(T).(T)`
        // Use cursor to get token stream, then convert to string
        let check_str = after_ret.cursor().token_stream().to_string();
        let is_type_assertion = check_str.contains(".(");
        
        // For chained assertions like `x.(int).(int)`, count the number of `.(...)` groups
        let paren_count = check_str.matches(".(").count();
        
        if is_type_assertion {
            // Parse receiver and all type assertion groups
            input.advance_to(&after_ret);
            let receiver_ident: Ident = input.parse()?;
            let receiver = Expr::Path(syn::ExprPath { attrs: vec![], qself: None, path: syn::Path::from(receiver_ident) });
            
            // Collect all type names from chained assertions
            let mut types: Vec<syn::Type> = Vec::new();
            loop {
                // Check if there's another `.(T)` group
                let next_fork = input.fork();
                if !next_fork.peek(syn::token::Dot) { break; }
                let _: proc_macro2::Punct = input.parse()?;
                let _: proc_macro2::Group = input.parse()?;
                // Extract type from the paren group
                let tfork = after_ret.fork();
                let mut all_groups: Vec<proc_macro2::Group> = Vec::new();
                let mut remaining = tfork;
                let _ = remaining.parse::<proc_macro2::TokenTree>()?; // skip receiver
                loop {
                    let mut gcheck = remaining.fork();
                    // Check for `.` punct
                    if let Ok(tt) = gcheck.parse::<proc_macro2::TokenTree>() {
                        if let proc_macro2::TokenTree::Punct(p) = tt {
                            if p.as_char() == '.' {
                                // Check for `(T)` group
                                let mut gcheck2 = gcheck.fork();
                                if let Ok(tt2) = gcheck2.parse::<proc_macro2::TokenTree>() {
                                    if let proc_macro2::TokenTree::Group(g) = tt2 {
                                        all_groups.push(g);
                                        remaining = gcheck2;
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                    break;
                }
                for g in &all_groups {
                    if g.delimiter() == proc_macro2::Delimiter::Parenthesis {
                        if let Ok(tid) = syn::parse2::<syn::Ident>(g.stream()) {
                            let tname = tid.to_string();
                            let rt = match tname.as_str() {
                                "int" => "i32", "int8" => "i8", "int16" => "i16",
                                "int32" => "i32", "int64" => "i64",
                                "uint" => "u32", "uint8" => "u8", "uint16" => "u16",
                                "uint32" => "u32", "uint64" => "u64",
                                "uintptr" => "usize", "byte" => "u8",
                                "rune" => "char", "float32" => "f32",
                                "float64" => "f64", "bool" => "bool",
                                "string" => "String",
                                "error" => "Box<dyn std::error::Error>",
                                _ => "i32",
                            };
                            types.push(syn::parse_str(rt).unwrap());
                        }
                    }
                }
                if !all_groups.is_empty() {
                    input.advance_to(&remaining);
                }
            }
            
            if types.is_empty() {
                // No types found, just return the receiver
                stmts.push(GoStmt::Expr(receiver));
            } else {
                // Build nested casts: ((x as T1) as T2) ...
                // Use GoTypeAssert which handles the transpilation in go_stmt_to_rust
                // to avoid syn::parse_quote mis-parsing &x as Expr::Return
                for ty in types.into_iter().rev() {
                    stmts.push(GoStmt::GoTypeAssert(receiver.clone(), ty.clone()));
                }
            }
            if input.peek(token::Semi) {
                let _semi: token::Semi = input.parse()?;
            }
            return Ok(true);
        }

        let expr_fork = after_ret.fork();
        if expr_fork.parse::<Expr>().is_ok() {
            input.advance_to(&after_ret);
            let first = input.parse::<Expr>()?;
            let multi_fork = input.fork();
            if multi_fork.peek(token::Comma) {
                // Multi-return: collect all expressions
                let mut multi_exprs: Vec<Expr> = vec![first];
                input.parse::<token::Comma>()?;
                loop {
                    if input.peek(syn::token::Brace) {
                        break;
                    }
                    let local_fork = input.fork();
                    // Stop at Go keywords that start new statements
                    if local_fork.peek(syn::Ident) {
                        let kw_fork = local_fork.fork();
                        if let Ok(kw) = kw_fork.parse::<syn::Ident>() {
                            let kw_str = kw.to_string();
                            if matches!(kw_str.as_str(),
                                "if" | "for" | "return" | "switch" | "case" | "default") {
                                break;
                            }
                        }
                    }
                    let next_fork = input.fork();
                    if next_fork.parse::<Expr>().is_ok() {
                        let expr = input.parse::<Expr>()?;
                        multi_exprs.push(expr);
                        if input.peek(token::Comma) {
                            input.parse::<token::Comma>()?;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                stmts.push(GoStmt::GoReturn(multi_exprs));
            } else {
                // Single return
                stmts.push(GoStmt::Expr(parse_quote! { return #first }));
            }
            if input.peek(token::Semi) {
                let _semi: token::Semi = input.parse()?;
            }
            return Ok(true);
        }
    }

    // `return` with no expression
    stmts.push(GoStmt::Expr(parse_quote! { return }));
    if input.peek(token::Semi) {
        let _semi: token::Semi = input.parse()?;
    }
    Ok(true)
}

// ─── Inline parse helpers used inside parse_go_block and free_fn ───────

/// Parse comma-separated expressions from a group (e.g., slice elements).
#[derive(Default)]
pub(crate) struct ElemParser {
    pub(crate) elems: Vec<Expr>,
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

/// Parse key:value map entries from a group (e.g., map literal contents).
#[derive(Default)]
pub(crate) struct MapEntryParser {
    pub(crate) entries: Vec<(Expr, Expr)>,
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

// ─── Helper functions for parsing slice/map elements ───────────────────

/// Parse key:value map entries from a token stream.
// ─── Free function: go_stmt_to_rust (bridging function) ────────────────

pub(crate) fn go_stmt_to_rust(stmt: &GoStmt) -> TokenStream {
    match stmt {
        GoStmt::Local(local) => {
            let pat = &local.pat;
            let val = local.init.as_ref().map(|v| go_to_rust(&v.expr));
            quote! { let #pat = #val; }
        }
        GoStmt::GoLocal(ident, val) => {
            quote! { let mut #ident = #val; }
        }
        GoStmt::If(go_if) => {
            let cond = go_to_rust(&go_if.cond);
            let then_body: Vec<_> = go_if.then_block.stmts.iter()
                .map(|s| go_stmt_to_rust(s)).collect();
            let then_block: Box<syn::ExprBlock> = syn::parse_quote!({ #(#then_body);* });
            let else_block = go_if.else_block.as_ref().map(|eb| {
                let else_body: Vec<_> = eb.stmts.iter().map(|s| go_stmt_to_rust(s)).collect();
                let block: Box<syn::ExprBlock> = syn::parse_quote!({ #(#else_body);* });
                quote! { else #block }
            });
            quote! { if #cond #then_block #else_block }
        }
        GoStmt::Expr(expr) => {
            go_to_rust(expr)
        }
        GoStmt::GoChannelSend(ch, val) => {
            let ch_rust = go_to_rust(ch);
            let val_rust = go_to_rust(val);
            quote! { #ch_rust.send(#val_rust); }
        }
        GoStmt::GoChannelRecv(ch) => {
            let ch_rust = go_to_rust(ch);
            quote! { return #ch_rust.recv().unwrap(); }
        }
        GoStmt::GoTypeAssert(receiver, ty) => {
            let recv_rust = go_to_rust(receiver);
            let ty_str = quote! { #ty }.to_string();
            match ty_str.as_str() {
                "String" => quote! { ::std::string::ToString::to_string(&#recv_rust) },
                "bool" => quote! { #recv_rust != 0 },
                "char" => quote! { (#recv_rust as u8) as char },
                _ => quote! { #recv_rust as #ty },
            }
        }

        GoStmt::GoSlice(elems) => {
            let elems: Vec<_> = elems.iter().map(go_to_rust).collect();
            quote! { vec![ #(#elems),* ] }
        }
        GoStmt::GoMap(ident, key_type, val_type, entries) => {
            go_stmt_to_rust_map(ident, key_type, val_type, entries)
        }
        GoStmt::GoReturn(exprs) => {
            // Multi-return: `return a, b` → `return (a, b)`
            if exprs.is_empty() {
                quote! { return }
            } else if exprs.len() == 1 {
                let e = &exprs[0];
                quote! { return #e }
            } else {
                let rust_exprs: Vec<_> = exprs.iter().map(go_to_rust).collect();
                quote! { return ( #(#rust_exprs),* ) }
            }
        }
        GoStmt::Switch(switch) => {
            super::free_fn::transpile_switch(switch)
        }
        GoStmt::Continue => {
            quote! { continue }
        }
        GoStmt::While(while_stmt) => {
            let cond = go_to_rust(&while_stmt.cond);
            let body: Vec<_> = while_stmt.body.stmts.iter()
                .map(|s| go_stmt_to_rust(s)).collect();
            quote! { while #cond { #(#body);* } }
        }
        GoStmt::GoFor(for_stmt) => {
            // iterable is now a Path, use directly in quote!
            let body: Vec<_> = for_stmt.body.stmts.iter()
                .map(|s| go_stmt_to_rust(s)).collect();
            let body_block: Box<syn::ExprBlock> = syn::parse_quote!({ #(#body);* });

            match (&for_stmt.init, &for_stmt.is_range) {
                (Some(GoForInit::Double(i, v)), true) => {
                    // `for i, v := range slice` → `for (i, v) in slice.iter().copied().enumerate()`
                    // .copied() turns &i32 into i32 so comparisons like `v > 0` work
                    let i_ident = i.clone();
                    let v_ident = v.clone();
                    let iterable = &for_stmt.iterable;
                    quote! {
                        for ( #i_ident, #v_ident ) in #iterable.iter().copied().enumerate() #body_block
                    }
                }
                (Some(GoForInit::Single(i)), true) => {
                    // `for i := range slice` → `for i in 0..slice.len()`
                    let i_ident = i.clone();
                    let iterable = &for_stmt.iterable;
                    quote! {
                        for #i_ident in 0.. #iterable.len() #body_block
                    }
                }
                (None, true) => {
                    // `for range slice` → `for _ in 0..slice.len()`
                    let iterable = &for_stmt.iterable;
                    quote! {
                        for _ in 0.. #iterable.len() #body_block
                    }
                }
                _ => {
                    // Fallback: should not happen for valid `for range`
                    dispatch::emit_todo("unsupported for form")
                }
            }
        }
    }
}

fn go_stmt_to_rust_map(
    ident: &str,
    key_type: &Option<Box<syn::Type>>,
    val_type: &Option<Box<syn::Type>>,
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

// ─── Block parsing helper (used by if statement parsing) ──────────────

/// Parse statements from a ParseStream without consuming braces.
/// Used by `if` statement parsing for nested then/else blocks.
pub(crate) fn parse_block_stmts(input: ParseStream) -> syn::Result<Vec<GoStmt>> {
    let mut stmts = Vec::new();
    while !input.is_empty() {
        // Check for Go-specific constructs first (maps, slices, if, switch, return)
        if parse_go_special_stmt(input, &mut stmts)? {
            continue; // Handled by the special case
        }
        // Fall back to the base parser for common statements
        parse_base_stmt(input, &mut stmts)?;
    }
    Ok(stmts)
}

/// Parse a single statement from a block - the common case shared between
/// `parse_block_stmts` and `parse_go_block`. Handles:
///   - `let` local declarations
///   - Go short declarations (`id := expr`)
///   - Standard expressions
///   - Skips tokens that don't match (to avoid infinite loops)
fn parse_base_stmt(input: ParseStream, stmts: &mut Vec<GoStmt>) -> syn::Result<()> {
    // 1. Try syn::Local (handles `let x = ...` declarations)
    let fork = input.fork();
    if fork.peek(syn::token::Let) {
        match fork.parse::<Stmt>() {
            Ok(Stmt::Local(_)) => {
                if let Ok(Stmt::Local(local)) = input.parse() {
                    stmts.push(GoStmt::Local(local));
                    if input.peek(token::Semi) {
                        let _semi: token::Semi = input.parse()?;
                    }
                    return Ok(());
                }
            }
            _ => {}
        }

        // Handle Go-style `let m = map[K]V{entries}` when syn can't parse
        let let_fork = input.fork();
        if let_fork.parse::<syn::token::Let>().is_ok()
            && let_fork.parse::<syn::Ident>().is_ok()
            && let_fork.parse::<syn::token::Eq>().is_ok()
        {
            if let_fork.peek(syn::Ident) {
                let map_fork = let_fork.fork();
                if let Ok(kw) = map_fork.parse::<syn::Ident>() {
                    if kw == "map" && map_fork.peek(syn::token::Bracket) {
                        // Parse: `let m = map[K]V{entries}`
                        input.parse::<syn::token::Let>()?;
                        let ident = input.parse::<syn::Ident>()?;
                        let ident_str = ident.to_string();
                        input.parse::<syn::token::Eq>()?;
                        // Use the map decl parser
                        return parse_go_map_decl(input, ident_str, stmts);
                    }
                }
            }
        }
    }

    // Handle Go short variable declaration: `id := expr`
    let fork = input.fork();
    if fork.peek(syn::Ident) {
        let id_fork = fork.fork();
        if id_fork.parse::<syn::Ident>().is_ok()
            && id_fork.parse::<syn::token::Colon>().is_ok()
            && id_fork.peek(syn::token::Eq)
        {
            let ident = input.parse::<syn::Ident>()?;
            let _: syn::token::Colon = input.parse()?;
            let _: syn::token::Eq = input.parse()?;
            // Check if the value is a Go map literal: `map[K]V{...}`
            let val_fork = input.fork();
            let map_fork = val_fork.fork();
            if let Ok(first_tt) = map_fork.parse::<proc_macro2::TokenTree>() {
                if let proc_macro2::TokenTree::Ident(map_kw) = &first_tt {
                    if *map_kw == "map" {
                        return parse_go_map_decl(input, ident.to_string(), stmts);
                    }
                }
            }
            // Standard value expression
            let val: Expr = input.parse()?;
            let val_rust = go_to_rust(&val);
            stmts.push(GoStmt::GoLocal(ident, val_rust));
            if input.peek(token::Semi) {
                let _semi: token::Semi = input.parse()?;
            }
            return Ok(());
        }
    }

    // Try standard expression parsing
    let fork = input.fork();
    if let Ok(expr) = fork.parse::<Expr>() {
        input.advance_to(&fork);
        stmts.push(GoStmt::Expr(expr));
        if input.peek(token::Semi) {
            let _semi: token::Semi = input.parse()?;
        }
        return Ok(());
    }

    // Nothing matched - skip one token tree to make progress
    let _ = input.parse::<proc_macro2::TokenTree>();
    Ok(())
}
