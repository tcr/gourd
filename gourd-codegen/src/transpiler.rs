use proc_macro2::TokenStream;
use quote::quote;
use syn::ext::IdentExt;
use syn::parse::{discouraged::Speculative, Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token;
use syn::{BinOp, Block, Expr, ExprArray, ExprBlock, ExprField, ExprForLoop, ExprIf, ExprIndex, ExprLoop, ExprMethodCall, ExprRange, ExprWhile, Ident, UnOp};

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
            match syn::parse2::<GoSliceLit>(tokens.clone()) {
                Ok(slice_lit) => go_to_rust_slice(&slice_lit).into(),
                Err(_) => {
                    // Try to parse as Go map literal: map[K]V{key: val, ...}
                    match syn::parse2::<GoMapLit>(tokens.clone()) {
                        Ok(map_lit) => go_to_rust_map(&map_lit).into(),
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

// ──────────────────────────────────────────────
// Go receiver function → Rust impl block
// ──────────────────────────────────────────────

/// Receiver parsing: (name Type) or (name *Type) where * means pointer receiver
struct Receiver {
    name: Ident,
    _ty: syn::Type,
    pointer: bool,  // true for `*Foo` → `&mut self`
}

impl Receiver {
    fn from_tokens(tokens: TokenStream) -> syn::Result<Self> {
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
struct ReceiverFn {
    recv: Receiver,
    ident: Ident,
    inputs: GoFnInputs,
    output: Option<GoFnOutput>,
    /// Parsed body statements as Go AST elements (single tree per statement)
    stmts: Vec<GoStmt>,
}

/// A Go statement (expression or local declaration)
enum GoStmt {
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
fn replace_receiver(expr: &Expr, recv_name: &Ident) -> Expr {
    match expr {
        Expr::Field(f) => {
            if let Expr::Path(ref base_path) = *f.base {
                if base_path.path.is_ident(recv_name) {
                    // f.recv_fieldname  →  self.fieldname
                    let member = f.member.clone();
                    return syn::parse_quote! { self.#member };
                }
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
                op: b.op.clone(),
                right: Box::new(replace_receiver(&b.right, recv_name)),
            })
        }
        Expr::Unary(u) => {
            Expr::Unary(syn::ExprUnary {
                attrs: Vec::new(),
                op: u.op.clone(),
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
            limits: r.limits.clone(),
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
                else_branch: i.else_branch.as_ref().map(|(e, block)| (e.clone(), Box::new(replace_receiver(block, recv_name)))),
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
                    syn::Stmt::Expr(v, s) => syn::Stmt::Expr(*Box::new(replace_receiver(v, recv_name)), s.clone()),
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
fn replace_receiver_local(local: &syn::Local, recv_name: &Ident) -> syn::Local {
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

// ──────────────────────────────────────────────
// Go slice and map literals
// ──────────────────────────────────────────────

/// Go slice literal: `[]Type{elem1, elem2, ...}`
/// Parsed from Go source inside expressions, transpiles to Rust `vec![elem1, elem2, ...]`.
pub struct GoSliceLit {
    #[allow(dead_code)]
    elem_type: Option<syn::Type>,
    pub elems: Vec<Expr>,
}

/// Go map literal: `map[K]V{key1: val1, key2: val2, ...}`
/// Parsed from Go source, transpiles to Rust `std::collections::HashMap`.
pub struct GoMapLit {
    #[allow(dead_code)] pub key_type: Option<syn::Type>,
    pub val_type: Option<syn::Type>,
    pub entries: Vec<(Expr, Expr)>,  // (key, value) pairs
}

impl Parse for GoSliceLit {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Parse `[]` (optionally with a type inside like `[]int`), then `{elems}`
        // First, try to parse a bracket group (could be empty `[]` or `[]Type`)
        let mut has_type = false;
        let _type_name: Option<syn::Type> = None;
        
        // Try to parse as Group (handles `[]` directly, or `[]int` as Group content)
        let fork = input.fork();
        match fork.parse::<proc_macro2::TokenStream>() {
            Ok(_) => {
                // Consumed — but this doesn't work: ParseStream::parse<T> 
                // for TokenStream isn't implemented.
                // Instead, use syn::bracketed! or handle the bracket manually.
            }
            Err(_) => {}
        }
        
        // Actually, check if this starts with `[` by looking at the first token
        if !input.peek(syn::token::Bracket) {
            return Err(input.error("expected Go slice literal starting with `[]` or `[Type]`"));
        }
        
        // Use syn::bracketed to handle `[]` or `[Type]`
        // `syn::bracketed!` parses `[...]` as a Group into `ParseBuffer`
        // The content can be empty (just `[]`) or contain a type.
        let bracket_content;
        let _ = syn::bracketed!(bracket_content in input);
        
        // Check if there's a type inside
        if !bracket_content.is_empty() {
            // Parse the element type (e.g. `int`, `string`)
            let _elem_type: syn::Type = bracket_content.parse()?;
            has_type = true;
        }
        
        // Now parse `{e1, e2, ...}`
        let brace_content;
        let _brace = syn::braced!(brace_content in input);
        
        let mut elems = Vec::new();
        if !brace_content.is_empty() {
            while !brace_content.is_empty() {
                let expr = syn::Expr::parse(&brace_content)?;
                elems.push(expr);
                // Consume optional comma
                if !brace_content.is_empty() && brace_content.peek(token::Comma) {
                    let _: token::Comma = brace_content.parse()?;
                } else {
                    break;
                }
            }
        }
        
        // If has_type, store it for type inference
        let elem_type = if has_type { Some(syn::Type::Path(syn::TypePath {
            path: syn::Path::from(Ident::new("dummy_type_for_inference", proc_macro2::Span::call_site())),
            qself: None,
        })) } else { None };
        
        Ok(GoSliceLit { elem_type, elems })
    }
}

impl Parse for GoMapLit {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        //Parse `map` keyword
        let kw: syn::Ident = input.call(syn::Ident::parse_any)?;
        let kw_str = kw.to_string();
        if kw_str != "map" {
            return Err(input.error("expected `map` keyword"));
        }

        // Parse `[K]V` — bracketed key type, then value type
        let bracket_content;
        let _bracket = syn::bracketed!(bracket_content in input);
        let key_type: syn::Type = bracket_content.parse()?;

        // Value type follows (could be `int`, `string`, or another identifier)
        let val_type: syn::Type = input.parse()?;

        // Parse `{key: val, key2: val2, ...}`
        let brace_content;
        let _brace = syn::braced!(brace_content in input);

        let mut entries = Vec::new();
        if !brace_content.is_empty() {
            // Speculatively parse entries: key: value patterns
            while !brace_content.is_empty() {
                let (key, value) = parse_map_entry(&brace_content)?;
                entries.push((key, value));
                // Consume optional comma
                if !brace_content.is_empty() && brace_content.peek(token::Comma) {
                    let _comma: token::Comma = brace_content.parse()?;
                } else {
                    break;
                }
            }
        }

        Ok(GoMapLit { key_type: Some(key_type), val_type: Some(val_type), entries })
    }
}

/// Parse a map entry: key: value pair
fn parse_map_entry(input: ParseStream) -> syn::Result<(Expr, Expr)> {
    // Key could be: path identifier, or literal
    let key: Expr = input.parse()?;
    // Expect colon separator
    let _: syn::token::Colon = input.parse()?;
    // Parse value
    let value: Expr = input.parse()?;
    Ok((key, value))
}

pub fn go_to_rust_slice(input: &GoSliceLit) -> TokenStream {
    let elems: Vec<_> = input.elems.iter().map(|e| go_to_rust(e)).collect();
    quote! { vec![ #(#elems),* ] }
}

pub fn go_to_rust_map(input: &GoMapLit) -> TokenStream {
    if input.entries.is_empty() {
        return quote! { std::collections::HashMap::new() };
    }

    let insertions: Vec<_> = input.entries.iter().map(|(k, v)| {
        let key = go_to_rust(k);
        let val = go_to_rust(v);
        quote! { m.insert(#key, #val); }
    }).collect();

    quote! { {
        let mut m = std::collections::HashMap::new();
        #(#insertions)*
        m
    } }
}

/// Top-level parse function for Go slice literals: `[]Type{...}` or `[]{...}`
/// This is called from the proc-macro entry point after checking the token shape.
pub fn parse_go_slice(tokens: &proc_macro2::TokenStream) -> syn::Result<GoSliceLit> {
    use proc_macro2::TokenTree;
    let mut iter = tokens.clone().into_iter();
    
    // First token must be a Group with Bracket delimiter (the `[]` part)
    match iter.next() {
        Some(TokenTree::Group(group)) if group.delimiter() == proc_macro2::Delimiter::Bracket => {
            // Group contains either empty `[]` or `[]int` etc.
            // Get the remaining tokens: `{elem1, elem2, ...}`
            let remaining: proc_macro2::TokenStream = iter.collect();
            
            // Construct a synthetic TokenStream that GoSliceLit::parse can work with:
             // Original: Group(Bracket, ...) + remaining (which starts with Group(Curly, ...))
            // We need to tell GoSliceLit's Parse impl: "first there's a bracket group, then a brace group"
             // Just rebuild: Group(Bracket, <contents>) + Group(Curly, <contents of brace group>)
            
            // The bracket group content is either empty or a type identifier
            let bracket_inner: proc_macro2::TokenStream = group.stream();
            
            // Now we need to extract the brace group content from `remaining`
            let brace_inner: proc_macro2::TokenStream = extract_brace_content(&remaining)?;
            
            // Reconstruct: [bracket] + {brace_content}
            let mut synthetic: proc_macro2::TokenStream = proc_macro2::TokenStream::new();
            
            // Bracket group — use the same content as the original (handles `[]` or `[]int`)
            synthetic.extend(Some(TokenTree::Group(proc_macro2::Group::new(
                proc_macro2::Delimiter::Bracket,
                bracket_inner,
            ))));
            // Brace group — use extracted content
            synthetic.extend(Some(TokenTree::Group(proc_macro2::Group::new(
                proc_macro2::Delimiter::Brace,
                brace_inner,
            ))));
            
            // Parse as GoSliceLit
            syn::parse2::<GoSliceLit>(synthetic).map_err(|e| {
                syn::Error::new(e.span(), format!("expected Go slice literal: {}", e))
            })
        }
        _ => Err(syn::Error::new(proc_macro2::Span::call_site(), "expected Go slice literal starting with `[]` or `[Type]`")),
    }
}

/// Top-level parse function for Go map literals: `map[K]V{key: val, ...}`
/// This is called from the proc-macro entry point after checking the token shape.
pub fn parse_go_map(tokens: &proc_macro2::TokenStream) -> syn::Result<GoMapLit> {
    use proc_macro2::TokenTree;
    
    // tokens already start with the `map` keyword (checked by lib.rs)
    let mut iter = tokens.clone().into_iter();
    
    // Skip the `map` ident
    match iter.next() {
        Some(TokenTree::Ident(id)) => {
            if id.to_string() != "map" {
                return Err(syn::Error::new(proc_macro2::Span::call_site(), "expected `map` keyword"));
            }
        }
        _ => return Err(syn::Error::new(proc_macro2::Span::call_site(), "expected `map` keyword")),
    }
    
    // Next comes `[K]` — a bracket group
    let group_tree: TokenTree = iter.next()
        .ok_or_else(|| syn::Error::new(proc_macro2::Span::call_site(), "expected `[K]` after `map`"))?;

    match group_tree {
        TokenTree::Group(group) if group.delimiter() == proc_macro2::Delimiter::Bracket => {
            let bracket_inner: proc_macro2::TokenStream = group.stream();
            
            // Remaining tokens: value type + `{entries}` (brace group)
            let remaining: proc_macro2::TokenStream = iter.collect();
            
            // Extract the brace content from `remaining`
            let brace_inner: proc_macro2::TokenStream = extract_brace_content(&remaining)?;
            
            // Parse the value type (everything before the brace group in `remaining`)
            let val_type: Option<syn::Type> = {
                let mut has_val_type = false;
                for tt in remaining.clone() {
                    if let TokenTree::Group(g) = &tt {
                        if g.delimiter() == proc_macro2::Delimiter::Brace {
                            break;
                        }
                    }
                    has_val_type = true;
                    // Collect type tokens (identifiers)
                }
                if !has_val_type {
                    None
                } else {
                    let val_stream: proc_macro2::TokenStream = remaining
                        .clone()
                        .into_iter()
                        .take_while(|tt| {
                            if let proc_macro2::TokenTree::Group(g) = tt {
                                g.delimiter() != proc_macro2::Delimiter::Brace
                            } else {
                                true
                            }
                        })
                        .collect();
                    Some(syn::parse2::<syn::Type>(val_stream).map_err(|e| {
                        syn::Error::new(e.span(), format!("expected value type in map: {}", e))
                    })?)
                }
            };
            
            // Construct a synthetic TokenStream that GoMapLit::parse can work with:
            // map ident + [bracket_key] valtype {entries}
            let mut synthetic: proc_macro2::TokenStream = proc_macro2::TokenStream::new();
            
            // Add the `map` keyword (already consumed and validated)
            synthetic.extend(Some(TokenTree::Ident(
                proc_macro2::Ident::new("map", proc_macro2::Span::call_site()),
            )));
            
            // Add the bracket group (key type)
            synthetic.extend(Some(TokenTree::Group(proc_macro2::Group::new(
                proc_macro2::Delimiter::Bracket,
                bracket_inner,
            ))));
            
            // Add the value type token(s) before the brace group
            // The val_type is either None (unnamed map) or the parsed type
            if let Some(vt) = val_type {
                // Insert val_type tokens after the bracket
                let val_stream: proc_macro2::TokenStream = quote! { #vt };
                synthetic.extend(val_stream);
            }
            
             // Add the brace group with entries
             synthetic.extend(Some(TokenTree::Group(proc_macro2::Group::new(
                 proc_macro2::Delimiter::Brace,
                 brace_inner,
             ))));
             
             // Parse as GoMapLit
             syn::parse2::<GoMapLit>(synthetic).map_err(|e| {
                 syn::Error::new(e.span(), format!("expected Go map literal: {}", e))
             })
         }
         _ => Err(syn::Error::new(proc_macro2::Span::call_site(), "expected `[K]` after `map`")),
     }
}

/// Extract the content (inner tokens) from the first Curly Group in a TokenStream.
fn extract_brace_content(tokens: &proc_macro2::TokenStream) -> syn::Result<proc_macro2::TokenStream> {
    use proc_macro2::TokenTree;
    
    for tt in tokens.clone() {
        if let TokenTree::Group(g) = tt {
            if g.delimiter() == proc_macro2::Delimiter::Brace {
                return Ok(g.stream());
            }
        }
    }
    Err(syn::Error::new(proc_macro2::Span::call_site(), "expected `{...}` braces"))
}

