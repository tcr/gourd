use proc_macro2::TokenStream;
use quote::quote;
use syn::ext::IdentExt;
use syn::parse::{discouraged::Speculative, Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token;
use syn::{BinOp, Block, Expr, ExprArray, ExprBlock, ExprField, ExprForLoop, ExprIf, ExprIndex, ExprLoop, ExprMethodCall, ExprRange, ExprWhile, Ident, UnOp};

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
        Expr::Array(e)     => transpile_array(e),
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
            // Try to parse as Go slice literal: []Type{...}
            match syn::parse2::<slices::GoSliceLit>(tokens.clone()) {
                Ok(slice_lit) => slices::go_to_rust_slice(&slice_lit).into(),
                Err(_) => {
                    // Try to parse as Go map literal: map[K]V{key: val, ...}
                    match syn::parse2::<slices::GoMapLit>(tokens.clone()) {
                        Ok(map_lit) => slices::go_to_rust_map(&map_lit).into(),
                        Err(_) => emit_todo("unsupported Go form"),
                    }
                }
            }
        }
        _                  => emit_todo("unsupported Go form"),
    }
}

/// Literals — string literals from Go become owned Rust Strings.
fn transpile_lit(input: &syn::ExprLit) -> TokenStream {
    let lit = &input.lit;
    match lit {
        syn::Lit::Str(s) => {
            // In Go, `s := "hello"` produces an owned string.
            // In Rust, `"hello"` is `&str`.  Convert to String.
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

/// Go assignment `x = y` → Rust `x = y` (for mutation after `let mut`)
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

/// `(x)  r  (x)`
fn transpile_paren(input: &syn::ExprParen) -> TokenStream {
    let inner  = go_to_rust(&input.expr);
    quote! { ( #inner ) }
}

fn transpile_index(input: &ExprIndex) -> TokenStream {
    let seq  = go_to_rust(&input.expr);
    let  idx  = go_to_rust(&input.index);
    quote! { #seq[ #idx ] }
}

fn transpile_array(input: &ExprArray) -> TokenStream {
    let elems: Vec<_> = input.elems.iter().map(go_to_rust).collect();
    quote! { [ #(#elems),* ] }
}

fn transpile_method_call(input: &ExprMethodCall) -> TokenStream {
    let receiver  = go_to_rust(&input.receiver);
    let method_name  = &input.method;
    let args: Vec<_>  = input.args.iter().map(go_to_rust).collect();
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

/// An `{ ... }` block: transpile each statement; the final expression
/// becomes the block's value.
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
                        // Convert `mut x` from Go pattern: the Pat already has mut binding stored in it
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

/// Parse type Name along w/ and shared type spec.
struct GoFnInputs {
    args: Vec<GoParam>,
}

struct GoParam {
    id: Ident,
    ty: Option<Box<syn::Type>>,
    slice_elem: Option<syn::Type>,
}

struct GoFnOutput {
    tys: Vec<syn::Type>,
}

struct GoFn {
    ident: Ident,
    generics: Punctuated<syn::GenericParam, token::Comma>,
    inputs: GoFnInputs,
    output: Option<GoFnOutput>,
    block: Block,
}

impl Parse for GoFnInputs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = Vec::new();
        while !input.is_empty() {
            let id: Ident = input.parse()?;
            let _ty: Option<Box<syn::Type>> = None;
            let mut group_ids: Vec<Ident> = Vec::new();

            // Collect group names: look for comma + named params sharing a type.
            // Stop if the name after the comma is a known Go type keyword.
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
                        // This comma is a param separator, not a group comma. Rollback.
                        input.advance_to(&peek_fork);
                        break;
                    }
                    // It's a group name: consume the comma and the name.
                    input.parse::<token::Comma>()?;
                    let param_name: Ident = input.parse()?;
                    group_ids.push(param_name);
                } else {
                    // `[` (slice type) — stop grouping.
                    break;
                }
            }

            // Parse the type that follows (for non-slice cases).
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
                // Slice type: `[]T` — parse element type from what follows the brackets.
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
        // Skip `->` if present (Rust-style return arrow)
        if input.peek(syn::token::RArrow) {
            let _: syn::token::RArrow = input.parse()?;
        }
        // Parse the return type(s).
        // If it's a tuple or single type, parse it. Then check for Go-style
        // additional comma-separated types (multi-return).
        if !input.peek(syn::token::Brace) {
            let t = input.parse()?;
            tys.push(t);
            // Go-style: `() (int, error)` or `(int, error)` — multi-returns
            // A comma means another return type (Go multi-return)
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
        // Parse `fn` keyword
        let _fn: Ident = input.call(Ident::parse_any)?;
        // Parse function name
        let ident: Ident = input.parse()?;
        // Parse generics
        let generics = Punctuated::<syn::GenericParam, token::Comma>::new();
        if input.peek(syn::token::Bracket) {
            let content;
            let _bracketed = syn::bracketed!(content in input);
            Punctuated::<syn::GenericParam, token::Comma>::parse_terminated(&content)?;
        }
        // Parse parameters
        let paren_content;
        let _paren = syn::parenthesized!(paren_content in input);
        let inputs = paren_content.parse()?;
        // Parse optional return
        let output = if !input.is_empty() {
            let outer = input.parse()?;
            Some(outer)
        } else {
            None
        };
        // Parse body
        let block: Block = input.parse()?;
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
            let seg = type_path.path.segments.last();
            match seg {
                Some(seg) => go_type_map(&seg.ident),
                None => quote! { #ty },
            }
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

            // Map return type (include `->` arrow)
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

            // Map parameters with Go-style shorthand support:
            //    func foo(a, b int) maps to: `a: i32, b: i32`
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

            // Transpile block statements
            let mut stmts = Vec::new();
            for stm in &go_fn.block.stmts {
                match stm {
                    syn::Stmt::Expr(val_expr, _) => {
                        stmts.push(go_to_rust(val_expr));
                    }
                    syn::Stmt::Local(local) => {
                        let local_pat = &local.pat;
                        let local_val = local.init.as_ref().map(|v| go_to_rust(&v.expr));
                        stmts.push(quote! { let #local_pat = #local_val; });
                    }
                    _ => stmts.push(emit_todo("statement not yet supported")),
                }
            }
            let body: Box<syn::ExprBlock> = syn::parse_quote! {{ #(#stmts);* }};

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

struct GoStruct {
    ident: Ident,
    fields: Vec<GoStructField>,
}

struct GoStructField {
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
