use proc_macro2::TokenStream;
use quote::quote;
use syn::ext::IdentExt;
use syn::parse::{discouraged::Speculative, Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token;
use syn::{BinOp, Expr, ExprArray, ExprBlock, ExprField, ExprForLoop, ExprIf, ExprIndex, ExprLoop, ExprMethodCall, ExprRange, ExprWhile, Ident, Stmt, UnOp};

pub mod funcs;
pub mod slices;

/// Emit a compile-time error for forms we don't support.
fn emit_todo(msg: &'static str) -> TokenStream {
    quote! { {  compile_error!( concat!("TODO: ", #msg) );  unreachable!()  }}
}

/// Dispatch the AST per expression node
pub fn go_to_rust(input: &Expr) -> TokenStream {
    match input {
        Expr::Lit(e)       => transpile_lit(e),
        Expr::Binary(e)    => transpile_binary(e),
        Expr::Unary(e)     => transpile_unary(e),
        Expr::Path(e)      => transpile_path(e),
        Expr::Call(e)      => transpile_call(e),
        Expr::Paren(e)     => transpile_paren(e),
        Expr::Group(e)     => go_to_rust(&e.expr),
        Expr::Block(e)     => transpile_block(e),
        Expr::If(e)        => transpile_if(e),
        Expr::Range(e)     => transpile_range(e),
        Expr::Index(e)     => transpile_index(e),
        Expr::Array(e)     => {
            // In Go slice literals like `[]int{ 1, 2, 3 }`, syn parses `[]`
            // as an empty array expression. If the array has no elements,
            // this is likely the start of a Go slice literal. We emit
            // an empty vec, but the actual slice elements come from the
            // `Expr::Verbatim` handling above when syn partially parses
            // the slice literal.
            if e.elems.is_empty() {
                quote! { vec![] }
            } else {
                // Normal Rust array literal — translate elements
                let elems: Vec<_> = e.elems.iter().map(go_to_rust).collect();
                quote! { [#(#elems),*] }
            }
        }
        Expr::Loop(e)      => transpile_loop(e),
        Expr::ForLoop(e)   => transpile_for_loop(e),
        Expr::While(e)     => transpile_while(e),
        Expr::MethodCall(c)=> transpile_method_call(c),
        Expr::Field(e)     => transpile_field(e),
        Expr::Let(e)       => transpile_let(e),
        Expr::Tuple(e)     => transpile_tuple(e),
        Expr::Cast(e)      => transpile_cast(e),
        Expr::Assign(e)     => transpile_assign(e),
        Expr::Break(e)      => transpile_break(e),
        Expr::Return(e)     => transpile_return(e),
        Expr::Verbatim(tokens) => {
            // syn may produce Verbatim tokens for Go slice/map literals
            // that it couldn't fully parse. Check if there's a brace group
            // in the verbatim tokens — if so, extract elements as a slice.
            use proc_macro2::TokenTree;

            // Look for a brace group in the verbatim tokens
            for tt in tokens.clone() {
                if let TokenTree::Group(g) = tt
                    && g.delimiter() == proc_macro2::Delimiter::Brace {
                    // Extract elements from the brace group
                    let mut elems = Vec::new();
                    let brace_content = g.stream();

                    // Collect tokens from the brace group and parse as expressions
                    // We can use syn::Expr::parse with a ParseStream created from
                    // the token stream via a custom parser
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

                    let parser: ElemParser = syn::parse2(brace_content).unwrap_or_default();
                    for expr in parser.elems {
                        elems.push(go_to_rust(&expr));
                    }
                    return quote! { vec![ #(#elems),* ] };
                }
            }

            // No brace group — emit raw tokens (simple literals)
            quote! { #tokens }
        }
        _                  => emit_todo("unsupported Go form"),
    }
}

/// Literals — string literals from Go become owned Rust Strings.
fn transpile_lit(input: &syn::ExprLit) -> TokenStream {
    let lit = &input.lit;
    match lit {
        syn::Lit::Str(s) => {
            quote! { ::std::string::String::from(#s) }
        }
        _ => quote! { #lit },
    }
}

/// Go path `nil` r Rust `Option::None`, `true`/`false` r Rust `true`/`false`.
fn transpile_path(input: &syn::ExprPath) -> TokenStream {
    let p = &input.path;
    match p.get_ident() {
        Some(ident) => match ident.to_string().as_str() {
            "nil"  => quote! { None },
            "true" => quote! { true },
            "false" => quote! { false },
            _      => quote! { #p },
        },
        None => quote! { #p },
    }
}

fn transpile_binary(input: &syn::ExprBinary) -> TokenStream {
    let lhs  = go_to_rust(&input.left);
    let rhs  = go_to_rust(&input.right);
    match input.op {
        BinOp::Add(_)      => quote! { #lhs + #rhs },
        BinOp::Sub(_)      => quote! { #lhs - #rhs },
        BinOp::Mul(_)      => quote! { #lhs * #rhs },
        BinOp::Div(_)      => quote! { #lhs / #rhs },
        BinOp::Rem(_)      => quote! { #lhs % #rhs },
        BinOp::And(_)      => quote! { #lhs && #rhs },
        BinOp::Or(_)       => quote! { #lhs || #rhs },
        BinOp::BitXor(_)   => quote! { #lhs ^ #rhs },
        BinOp::BitAnd(_)   => quote! { #lhs & #rhs },
        BinOp::BitOr(_)    => quote! { #lhs | #rhs },
        BinOp::Shl(_)      => quote! { #lhs << #rhs },
        BinOp::Shr(_)      => quote! { #lhs >> #rhs },
        BinOp::Eq(_)       => quote! { #lhs == #rhs },
        BinOp::Ne(_)       => quote! { #lhs != #rhs },
        BinOp::Ge(_)       => quote! { #lhs >= #rhs },
        BinOp::Gt(_)       => quote! { #lhs > #rhs },
        BinOp::Le(_)       => quote! { #lhs <= #rhs },
        BinOp::Lt(_)       => quote! { #lhs < #rhs },
        _  => emit_todo("unsupported binary operator"),
    }
}

fn transpile_unary(input: &syn::ExprUnary) -> TokenStream {
    let inner = go_to_rust(&input.expr);
    match &input.op {
        UnOp::Not(_)  => quote! { ! #inner },
        UnOp::Neg(_)  => quote! { - #inner },
        UnOp::Deref(_) => quote! { * #inner },
        _             => emit_todo("unsupported unary operator"),
    }
}

/// Go `x := y` = Rust `let x = y`
fn transpile_let(input: &syn::ExprLet) -> TokenStream {
    let pat = &input.pat;
    let expr  = go_to_rust(&input.expr);
    quote! { let #pat = #expr }
}

/// Go tuple `(a, b)` → Rust tuple `(a, b)`
fn transpile_tuple(input: &syn::ExprTuple) -> TokenStream {
    let elems: Vec<_> = input.elems.iter().map(go_to_rust).collect();
    match elems.len() {
        0 => quote! { () },
        _ => quote! { ( #(#elems),* ) },
    }
}

/// Go `x as T` → Rust `x as T`
fn transpile_cast(input: &syn::ExprCast) -> TokenStream {
    let expr = go_to_rust(&input.expr);
    let ty = &input.ty;
    quote! { #expr as #ty }
}

/// Go assignment `x = y` → Rust `x = y`
fn transpile_assign(input: &syn::ExprAssign) -> TokenStream {
    let lhs = go_to_rust(&input.left);
    let rhs = go_to_rust(&input.right);
    quote! { #lhs = #rhs }
}

/// Go `break` labels/expressions
fn transpile_break(input: &syn::ExprBreak) -> TokenStream {
    let label = input.label.as_ref().map(|l| quote! { #l });
    let expr = input.expr.as_ref().map(|e| go_to_rust(e));
    match expr {
        Some(e) => quote! { break #label #e },
        None => quote! { break #label },
    }
}

/// Go `return` → Rust `return expr`
fn transpile_return(input: &syn::ExprReturn) -> TokenStream {
    let expr = input.expr.as_ref().map(|e| go_to_rust(e));
    match expr {
        Some(e) => quote! { return #e },
        None => quote! { return },
    }
}

/// Go `len(slice)` or `cap(slice)` r Rust `slice.len() as i32``
fn transpile_call(input: &syn::ExprCall) -> TokenStream {
    let args: Vec<_> = input.args.iter().map(go_to_rust).collect();
    if let Expr::Path(path) = &*input.func
        && let Some(name) = path.path.get_ident()
        && matches!(name.to_string().as_str(), "len" | "cap")
    {
        let arg = args[0].clone();
        return quote! { #arg.len() as i32 };
    }
    if let Expr::Path(path) = &*input.func
        && let Some(name) = path.path.get_ident()
        && name.to_string().as_str() == "string"
    {
        let arg = args[0].clone();
        return quote! { std::str::from_utf8(&#arg).unwrap_or("").to_string() };
    }
    let func = go_to_rust(&input.func);
    quote! { #func( #(#args),* ) }
}

fn transpile_paren(input: &syn::ExprParen) -> TokenStream {
    let inner  = go_to_rust(&input.expr);
    quote! { ( #inner ) }
}

fn transpile_index(input: &ExprIndex) -> TokenStream {
    let seq  = go_to_rust(&input.expr);
    let  idx  = go_to_rust(&input.index);
    quote! { #seq[ #idx ] }
}

#[allow(dead_code)]
fn transpile_array(input: &ExprArray) -> TokenStream {
    let elems: Vec<_> = input.elems.iter().map(go_to_rust).collect();
    quote! { [ #(#elems),* ] }
}

fn transpile_method_call(input: &ExprMethodCall) -> TokenStream {
    let receiver  = go_to_rust(&input.receiver);
    let method_name  = &input.method;
    let args: Vec<_>  = input.args.iter().map(go_to_rust).collect();
    // For `.get(key)` calls, wrap the key in a reference
    let args_str = method_name.to_string();
    if args_str == "get" {
        if let Some(first) = args.first() {
            let rest: Vec<_> = args.iter().skip(1).cloned().collect();
            return quote! { #receiver.#method_name( &#first #(#rest),* ) };
        }
    }
    quote! { #receiver.#method_name( #(#args),* ) }
}

fn transpile_field(input: &ExprField) -> TokenStream {
    let base   = go_to_rust(&input.base);
    let field  = &input.member;
    quote! { #base.#field }
}

fn transpile_loop(input: &ExprLoop) -> TokenStream {
    let label =  input.label.as_ref().map(|l| quote! { #l });
    let body  = &input.body;
    quote! { loop #label #body }
}

fn transpile_for_loop(input: &ExprForLoop) -> TokenStream {
    let pat  = &input.pat;
    let expr  = go_to_rust(&input.expr);
    let body  = &input.body;
    quote! { for #pat in #expr #body }
}

fn transpile_while(input: &ExprWhile) -> TokenStream {
    let label  =  input.label.as_ref().map(|l| quote! { #l });
    let cond  = go_to_rust(&input.cond);
    let body  = &input.body;
    quote! { while #cond #label #body }
}

fn transpile_range(input: &ExprRange) -> TokenStream {
    let _start =  input.start.as_ref().map(|e| go_to_rust(e));
    let  end  =  input.end.as_ref().map(|e| go_to_rust(e));
    let limits  = match input.limits {
        syn::RangeLimits::HalfOpen(_)  => quote! { .. },
        syn::RangeLimits::Closed(_)    => quote! { ..= },
    };
    match (input.start.as_ref(), input.end.as_ref()) {
        (Some(fd), Some(_ld))  => quote! { #fd #limits #end },
        (Some(e), None)   => quote! { #e #limits },
        (None, Some(e))   => quote! { #limits #e },
        (None, None)      => quote! { #limits },
    }
}

fn transpile_if(input: &ExprIf) -> TokenStream {
    let cond  = go_to_rust(&input.cond);
    let  then_block  = &input.then_branch;
    let else_block  =  input.else_branch.as_ref().map(|(_, e)| {
        let  e  =  go_to_rust(e);
        quote! { else { #e } }
    });
    quote! { if #cond #then_block #else_block }
}

fn transpile_block(input: &ExprBlock) -> TokenStream {
    if input.block.stmts.is_empty() {
        return quote! {{  }};
    }
    let mut outputs  = Vec::new();
    for stm  in input.block.stmts.iter() {
        match stm {
            syn::Stmt::Expr(val_expr, _)  => {
                outputs.push(go_to_rust(val_expr));
            }
                    syn::Stmt::Local(local)  => {
                        let local_pat  = &local.pat;
                        let local_val  = local.init.as_ref().map(|v| go_to_rust(&v.expr));
                        outputs.push(quote! { let #local_pat = #local_val; });
                    }
            _  => {
                return emit_todo("statement not yet supported");
            }
        }
    }
    quote! {{  { #(#outputs);* }  }}
}

// ──────────────────────────────────────────────
// Go function declaration → Rust fn (the `go!` macro)
// ──────────────────────────────────────────────

pub(crate) enum GoStmt {
    Local(syn::Local),
    Expr(Expr),
    GoSlice(Vec<Expr>),
    GoMap(String, Option<syn::Type>, Option<syn::Type>, Vec<(Expr, Expr)>),  // (ident, key_type, val_type, entries)
}

pub(crate) struct GoBlock {
    stmts: Vec<GoStmt>,
}

pub(crate) struct GoFnInputs {
    args: Vec<GoParam>,
}

pub(crate) struct GoParam {
    id: Ident,
    ty: Option<Box<syn::Type>>,
    slice_elem: Option<syn::Type>,
}

pub(crate) struct GoFnOutput {
    tys: Vec<syn::Type>,
}

struct GoFn {
    ident: Ident,
    generics: Punctuated<syn::GenericParam, token::Comma>,
    inputs: GoFnInputs,
    output: Option<GoFnOutput>,
    block: GoBlock,
}

impl Parse for GoFnInputs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = Vec::new();
        while !input.is_empty() {
            let id: Ident = input.parse()?;
            let _ty: Option<Box<syn::Type>> = None;
            let mut group_ids: Vec<Ident> = Vec::new();

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

/// Parse a Go slice literal pattern: []Type{e1, e2, ...} or []{e1, e2}
#[allow(dead_code)]
fn parse_go_slice_literal(input: ParseStream) -> syn::Result<Vec<Expr>> {
    // Consume first `[]` using bracketed! macro
    let _bracket_token;
    let bracket_content;
    _bracket_token = syn::bracketed!(bracket_content in input);
    // Skip optional type inside brackets (e.g. `[]int`)
    if !bracket_content.is_empty() {
        let _skip: syn::Type = bracket_content.parse()?;
    }
    if input.peek(syn::token::Brace) {
        let brace_content;
        let _ = syn::braced!(brace_content in input);
        let mut elems = Vec::new();
        if !brace_content.is_empty() {
            let fork = brace_content.fork();
            while !fork.is_empty() {
                if let Ok(e) = fork.parse::<Expr>() {
                    elems.push(e);
                    if fork.peek(token::Comma) {
                        fork.parse::<token::Comma>()?;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        Ok(elems)
    } else {
        Ok(Vec::new())
    }
}

/// Parse a Go map literal pattern: map[K]V{key: val, ...}
#[allow(dead_code)]
fn parse_go_map_literal(input: ParseStream) -> syn::Result<Vec<(Expr, Expr)>> {
    // `map` keyword already consumed
    let bracket_content;
    let _ = syn::bracketed!(bracket_content in input);
    // Parse K (key type) — skip if not a valid Rust type
    if !bracket_content.is_empty() {
        let _skip = bracket_content.parse::<syn::Type>();
    }
    // Parse V (value type) — skip if not a valid Rust type
    if input.peek(syn::Ident) || input.peek(syn::token::Bracket) {
        let _skip = input.parse::<syn::Type>();
    }
    // Parse `{key: val, ...}`
    let brace_content;
    let _ = syn::braced!(brace_content in input);
    let mut entries = Vec::new();
    if !brace_content.is_empty() {
        let fork = brace_content.fork();
        while !fork.is_empty() {
            if let Ok(key) = fork.parse::<Expr>() {
                if fork.peek(syn::token::Colon) {
                    let _: token::Colon = fork.parse()?;
                    if let Ok(value) = fork.parse::<Expr>() {
                        entries.push((key, value));
                        if fork.peek(token::Comma) {
                            fork.parse::<token::Comma>()?;
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
    }
    Ok(entries)
}

/// Parse each statement from the brace content using speculative parsing.
/// Handles Go slice/map literals that syn::Stmt can't capture.
fn parse_go_block(input: ParseStream) -> syn::Result<GoBlock> {
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
                    // parse failed — fall through to let fallback
                }
                _ => {}
            }
            // Handle `let m = map[K]V{...}` when syn can't parse the value
            let let_fork = brace_content.fork();
            if let_fork.parse::<syn::token::Let>().is_ok()
                && let_fork.parse::<syn::Ident>().is_ok()
                && let_fork.parse::<syn::token::Eq>().is_ok()
            {
                // Check if the value is a Go map literal
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
                                    let _: proc_macro2::TokenStream = k_content.parse()?;
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
                                    let _: proc_macro2::TokenStream = brace_content.parse()?;
                                    None
                                }
                            } else {
                                None
                            };
                            // Skip the `{...}` group and parse entries from it.
                            // Use step() to extract the group content from the main
                            // stream, which works on nested groups.
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
                                    // Parse entries from the inner stream using a custom parser
                                    #[derive(Default)]
                                    struct MapEntryParser { entries: Vec<(Expr, Expr)> }
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
                                    let parser: MapEntryParser = syn::parse2(inner_ts).unwrap_or_default();
                                    entries = parser.entries;
                                }
                                stmts.push(GoStmt::GoMap(ident_str.clone(), key_type.clone(), val_type.clone(), entries.clone()));
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
        //    (must come BEFORE syn::Expr parse since [] is valid Rust empty array)
        let fork = brace_content.fork();
        if fork.peek(syn::token::Bracket) {
            // Parse brackets from fork, then advance brace_content if it's a slice literal
            let _bracket;
            let bracket_content;
            _bracket = syn::bracketed!(bracket_content in fork);

            // Skip optional type inside first `[]` (e.g. `[int]`)
            if !bracket_content.is_empty() {
                let type_fork = bracket_content.fork();
                if type_fork.parse::<syn::Type>().is_ok() {
                    let _: syn::Type = bracket_content.parse()?;
                } else {
                    let _: proc_macro2::TokenStream = bracket_content.parse()?;
                }
            }

            // Also skip optional type after `[]` (e.g. `[]int` → skip `int`)
            // The type follows the closing bracket, not inside it
            let type_fork = fork.fork();
            if type_fork.parse::<syn::Type>().is_ok() {
                let _: syn::Type = fork.parse()?;
            }

            // Now check if fork points to `{` (after brackets and optional type)
            if fork.peek(syn::token::Brace) {
                use proc_macro2::TokenTree;
                // Advance the main stream to fork's position (at `{`)
                brace_content.advance_to(&fork);
                // Extract the inner content from the fork's token stream
                let group_stream: proc_macro2::TokenStream = fork.parse()?;
                if let Some(TokenTree::Group(group)) = group_stream
                    .into_iter()
                    .next()
                    .and_then(|t| {
                        if let TokenTree::Group(g) = t { Some(TokenTree::Group(g)) } else { None }
                    })
                    && group.delimiter() == proc_macro2::Delimiter::Brace
                {
                    // Parse elements from the group's inner stream
                    let inner_ts = group.stream();
                    let mut elems = Vec::new();
                    if !inner_ts.is_empty() {
                        #[derive(Default)]
                        struct ElemParser { elems: Vec<Expr> }
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
                        let parser: ElemParser = syn::parse2(inner_ts).unwrap_or_default();
                        elems = parser.elems;
                    }
                    // Advance brace_content to fork's position (at `{...}`).
                    // The `[]` and optional type were already handled by the
                    // fork-based detection above. Just skip past the brace group.
                    // `step()` with `.group(Delimiter::Brace)` works on nested
                    // groups because it extracts groups at the cursor level.
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
            // Not a slice literal — fall through to other parsers
        }

        // 3. Try standard expression parsing via speculative parse
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
                    // This is a Go map literal: map[K]V{...}
                    brace_content.parse::<syn::Ident>()?;
                    // Consume `[K]`
                    let k_content;
                    let _ = syn::bracketed!(k_content in brace_content);
                    // Parse K (key type) — skip if not a valid Rust type
                    if !k_content.is_empty() {
                        if k_content.parse::<syn::Type>().is_err() {
                            let _: proc_macro2::TokenStream = k_content.parse()?;
                        }
                    }
                    // Parse V (value type) — skip if not a valid Rust type
                    if brace_content.peek(syn::Ident) || brace_content.peek(syn::token::Bracket) {
                        if brace_content.parse::<syn::Type>().is_err() {
                            let _: proc_macro2::TokenStream = brace_content.parse()?;
                        }
                    }
                    // Parse `{key: val, ...}`
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
                            #[derive(Default)]
                            struct MapEntryParser { entries: Vec<(Expr, Expr)> }
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
                            let parser: MapEntryParser = syn::parse2(inner_ts).unwrap_or_default();
                            entries = parser.entries;
                        }
                        stmts.push(GoStmt::GoMap(String::new(), None, None, entries));
                        continue;
                    }
                }
            }
        }

        // 5. Nothing matched — try to skip one statement to avoid infinite loop
        let _ = brace_content.parse::<syn::Expr>();
    }

    Ok(GoBlock { stmts })
}
fn go_stmt_to_rust(stmt: &GoStmt) -> TokenStream {
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
            if entries.is_empty() {
                if ident.is_empty() {
                    return quote! { std::collections::HashMap::default() };
                }
                let name: syn::Ident = syn::parse_str(&ident).unwrap();
                if let (Some(kt), Some(vt)) = (&key_type, &val_type) {
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
            let block = if let (Some(kt), Some(vt)) = (&key_type, &val_type) {
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
                let name: syn::Ident = syn::parse_str(&ident).unwrap();
                quote! { let #name = #block; }
            }
        }
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

/// Map a single Go type identifier to its Rust equivalent.
fn go_type_map(ident: &syn::Ident) -> TokenStream {
    let name = ident.to_string();
    match name.as_str() {
        "bool"    => quote! { bool },
        "string"  => quote! { String },
        "int"     => quote! { i32 },
        "int8"    => quote! { i8 },
        "int16"   => quote! { i16 },
        "int32"   => quote! { i32 },
        "int64"   => quote! { i64 },
        "uint"    => quote! { u32 },
        "uint8"   => quote! { u8 },
        "uint16"  => quote! { u16 },
        "uint32"  => quote! { u32 },
        "uint64"  => quote! { u64 },
        "uintptr" => quote! { usize },
        "byte"    => quote! { u8 },
        "rune"    => quote! { char },
        "float32" => quote! { f32 },
        "float64" => quote! { f64 },
        "error"   => quote! { Box<dyn std::error::Error> },
        _ => quote! { #ident },
    }
}

/// Map Go type names to Rust, handling Path and composite types.
fn map_go_types(ty: &syn::Type) -> TokenStream {
    match ty {
        syn::Type::Path(type_path) => {
            // Only map if the first segment is a Go builtin
            if let Some(first) = type_path.path.segments.first() {
                let first_name = first.ident.to_string();
                if matches!(first_name.as_str(),
                    "bool" | "string" | "int" | "int8" | "int16" | "int32" | "int64"
                    | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr"
                    | "byte" | "rune" | "float32" | "float64" | "error"
                ) {
                    return go_type_map(&first.ident);
                }
            }
            // Not a Go builtin — pass through unchanged
            quote! { #ty }
        }
        syn::Type::Reference(type_ref) => {
            let elem = map_go_types(&type_ref.elem);
            match &type_ref.lifetime {
                Some(l) => quote! { & #l #elem },
                None => quote! { &#elem }
            }
        }
        syn::Type::Slice(type_array) => {
            let elem = map_go_types(&type_array.elem);
            quote! { &[ #elem ]}
        }
        syn::Type::Array(a) => {
            let elem = map_go_types(&a.elem);
            quote! { [ #elem; #a.len ] }
        }
        syn::Type::Tuple(type_tuple) => {
            let elems: Vec<_> = type_tuple.elems.iter().map(map_go_types).collect();
            match elems.len() {
                1 => quote! { ( #(#elems),* ) },
                0 => quote! { () },
                _ => quote! { ( #(#elems),* ) },
            }
        }
        syn::Type::Paren(inner) => {
            let mapped = map_go_types(&inner.elem);
            quote! { ( #mapped ) }
        }
        _ => quote! { #ty },
    }
}

/// Top-level: parse and transpile a Go function declaration to Rust.
pub fn go_to_rust_fn(input: TokenStream) -> TokenStream {
    match syn::parse2::<GoFn>(input) {
        Ok(go_fn) => {
            let fn_name = &go_fn.ident;
            let generics = &go_fn.generics;

            let output = go_fn.output.as_ref().map(|output| {
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

            let mut all_params = Vec::<TokenStream>::new();
            for param in &go_fn.inputs.args {
                let id = &param.id;
                match (&param.ty, &param.slice_elem) {
                    (None, None) => {
                        all_params.push(quote! { #id });
                    }
                    (_, Some(slice_inner)) => {
                        let mapped = map_go_types(slice_inner);
                        all_params.push(quote! { #id: &[ #mapped ]});
                    }
                    (Some(ty), None) => {
                        let mapped = map_go_types(ty);
                        all_params.push(quote! { #id: #mapped });
                    }
                }
            }

            let mut stmts = Vec::new();
            for stm in &go_fn.block.stmts {
                stmts.push(go_stmt_to_rust(stm));
            }
            let body: Box<syn::ExprBlock> = syn::parse_quote!({ #(#stmts);* });

            quote! {
                fn #fn_name #generics ( #(#all_params),* ) #output #body
            }
        }
        Err(e) => e.to_compile_error(),
    }
}

// ──────────────────────────────────────────────
// Go struct declaration → Rust struct
// ──────────────────────────────────────────────

pub(crate) struct GoStruct {
    ident: Ident,
    fields: Vec<GoStructField>,
}

pub(crate) struct GoStructField {
    name: Ident,
    ty: syn::Type,
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

pub fn go_to_rust_struct(input: TokenStream) -> TokenStream {
    match syn::parse2::<GoStruct>(input) {
        Ok(go_struct) => {
            let name = &go_struct.ident;
            let fields = go_struct.fields.iter().map(|f| {
                let fname = &f.name;
                let ftty = map_go_types(&f.ty);
                quote! { pub #fname: #ftty }
            });
            quote! {
                struct #name {
                    #(#fields),*
                }
            }
        }
        Err(e) => e.to_compile_error(),
    }
}
