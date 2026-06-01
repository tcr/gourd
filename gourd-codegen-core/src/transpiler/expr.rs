//! Expression-level Go → Rust transpilation.
//!
//! Converts `syn::Expr` AST nodes into `TokenStream` fragments via
//! recursive descent. Each `Expr` variant has a corresponding
//! `transpile_*` handler.

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Parse;
use syn::{BinOp, Expr, ExprArray, ExprBlock, ExprField, ExprForLoop, ExprIf, ExprIndex, ExprLoop, ExprMethodCall, ExprRange, ExprWhile, UnOp};

/// Emit a compile-time error for forms we don't support.
pub(crate) fn emit_todo(msg: &'static str) -> TokenStream {
    quote! { {
        compile_error!(concat!("TODO: ", #msg));
        unreachable!()
    }}
}

/// Dispatch the AST per expression node.
pub fn go_to_rust(input: &Expr) -> TokenStream {
    match input {
        Expr::Lit(e)        => transpile_lit(e),
        Expr::Binary(e)     => transpile_binary(e),
        Expr::Unary(e)      => transpile_unary(e),
        Expr::Path(e)       => transpile_path(e),
        Expr::Call(e)       => transpile_call(e),
        Expr::Paren(e)      => transpile_paren(e),
        Expr::Group(e)      => go_to_rust(&e.expr),
        Expr::Block(e)      => transpile_block(e),
        Expr::If(e)         => transpile_if(e),
        Expr::Range(e)      => transpile_range(e),
        Expr::Index(e)      => transpile_index(e),
        Expr::Array(e)      => transpile_array(e),
        Expr::Loop(e)       => transpile_loop(e),
        Expr::ForLoop(e)    => transpile_for_loop(e),
        Expr::While(e)      => transpile_while(e),
        Expr::MethodCall(c) => transpile_method_call(c),
        Expr::Field(e)      => transpile_field(e),
        Expr::Let(e)        => transpile_let(e),
        Expr::Tuple(e)      => transpile_tuple(e),
        Expr::Cast(e)       => transpile_cast(e),
        Expr::Assign(e)     => transpile_assign(e),
        Expr::Break(e)        => transpile_break(e),
        Expr::Return(e)       => transpile_return(e),
        Expr::Macro(e)        => go_to_rust_macro(e),
        Expr::Verbatim(tokens) => transpile_verbatim(tokens),
        _                     => emit_todo("unsupported Go form"),
    }
}

/// Expression transpilation for Rust **match patterns**.
///
/// Unlike `go_to_rust`, this keeps string literals as `&str` patterns
/// (raw `"..."` literal) instead of wrapping them in `String::from(...)`,
/// because Rust match arms require patterns, not expressions.
pub fn go_to_rust_pattern(input: &Expr) -> TokenStream {
    match input {
        Expr::Lit(e)        => transpile_lit_pattern(e),
        Expr::Path(e)       => transpile_path(e),
        Expr::Paren(e)      => go_to_rust_pattern(&e.expr),
        Expr::Group(e)      => go_to_rust_pattern(&e.expr),
        Expr::Tuple(e)      => transpile_tuple(e),
        Expr::Verbatim(tokens) => transpile_verbatim(tokens),
        _                   => emit_todo("unsupported match pattern"),
    }
}


// ─── Individual handlers ───────────────────────────────────────────────

fn transpile_lit(input: &syn::ExprLit) -> TokenStream {
    let lit = &input.lit;
    match lit {
        syn::Lit::Str(s) => quote! { ::std::string::String::from(#s) },
        _                => quote! { #lit },
    }
}

/// Pattern variant: keep string literals as `&str` patterns.
pub fn transpile_lit_pattern(input: &syn::ExprLit) -> TokenStream {
    let lit = &input.lit;
    match lit {
        syn::Lit::Str(s) => quote! { #s },  // &str pattern, not String::from
        _                => quote! { #lit },
    }
}

fn transpile_path(input: &syn::ExprPath) -> TokenStream {
    let p = &input.path;
    match p.get_ident() {
        Some(ident) => match ident.to_string().as_str() {
            "nil"   => quote! { None },
            "true"  => quote! { true },
            "false" => quote! { false },
            _       => quote! { #p },
        },
        None => quote! { #p },
    }
}

fn transpile_binary(input: &syn::ExprBinary) -> TokenStream {
    let lhs = go_to_rust(&input.left);
    let rhs = go_to_rust(&input.right);
    match input.op {
        BinOp::Add(_)     => quote! { #lhs + #rhs },
        BinOp::Sub(_)     => quote! { #lhs - #rhs },
        BinOp::Mul(_)     => quote! { #lhs * #rhs },
        BinOp::Div(_)     => quote! { #lhs / #rhs },
        BinOp::Rem(_)     => quote! { #lhs % #rhs },
        BinOp::And(_)     => quote! { #lhs && #rhs },
        BinOp::Or(_)      => quote! { #lhs || #rhs },
        BinOp::BitXor(_)  => quote! { #lhs ^ #rhs },
        BinOp::BitAnd(_)  => quote! { #lhs & #rhs },
        BinOp::BitOr(_)   => quote! { #lhs | #rhs },
        BinOp::Shl(_)     => quote! { #lhs << #rhs },
        BinOp::Shr(_)     => quote! { #lhs >> #rhs },
        BinOp::Eq(_)      => quote! { #lhs == #rhs },
        BinOp::Ne(_)      => quote! { #lhs != #rhs },
        BinOp::Ge(_)      => quote! { #lhs >= #rhs },
        BinOp::Gt(_)      => quote! { #lhs > #rhs },
        BinOp::Le(_)      => quote! { #lhs <= #rhs },
        BinOp::Lt(_)      => quote! { #lhs < #rhs },
        _                 => emit_todo("unsupported binary operator"),
    }
}

fn transpile_unary(input: &syn::ExprUnary) -> TokenStream {
    let inner = go_to_rust(&input.expr);
    match &input.op {
        UnOp::Not(_)    => quote! { ! #inner },
        UnOp::Neg(_)    => quote! { - #inner },
        UnOp::Deref(_)  => quote! { * #inner },
        _               => emit_todo("unsupported unary operator"),
    }
}

fn transpile_let(input: &syn::ExprLet) -> TokenStream {
    let pat = &input.pat;
    let expr = go_to_rust(&input.expr);
    quote! { let #pat = #expr }
}

fn transpile_tuple(input: &syn::ExprTuple) -> TokenStream {
    let elems: Vec<_> = input.elems.iter().map(go_to_rust).collect();
    match elems.len() {
        0 => quote! { () },
        _ => quote! { ( #(#elems),* ) },
    }
}

fn transpile_cast(input: &syn::ExprCast) -> TokenStream {
    let expr = go_to_rust(&input.expr);
    let ty = &input.ty;
    quote! { #expr as #ty }
}

fn transpile_assign(input: &syn::ExprAssign) -> TokenStream {
    let lhs = go_to_rust(&input.left);
    let rhs = go_to_rust(&input.right);
    quote! { #lhs = #rhs }
}

fn transpile_break(input: &syn::ExprBreak) -> TokenStream {
    let label = input.label.as_ref().map(|l| quote! { #l });
    let expr = input.expr.as_ref().map(|e| go_to_rust(e));
    match expr {
        Some(e) => quote! { break #label #e },
        None => quote! { break #label },
    }
}

fn transpile_return(input: &syn::ExprReturn) -> TokenStream {
    let expr = input.expr.as_ref().map(|e| go_to_rust(e));
    match expr {
        Some(e) => quote! { return #e },
        None => quote! { return },
    }
}

/// Handle Rust macro invocations (e.g. `vec![...]`) passed through `quote!`.
/// These are valid Rust already — just emit the macro tokens as-is.
fn go_to_rust_macro(input: &syn::ExprMacro) -> TokenStream {
    quote! { #input }
}

fn transpile_call(input: &syn::ExprCall) -> TokenStream {
    let args: Vec<_> = input.args.iter().map(go_to_rust).collect();
    if let Expr::Path(path) = &*input.func
        && let Some(name) = path.path.get_ident()
        && matches!(name.to_string().as_str(), "len" | "cap")
    {
        let arg = args[0].clone();
        return quote! { #arg.len() as i32 };
    }
    // Go type conversion calls: int(), int8(), ..., string(), bool(), byte(), rune(), ...
    if let Expr::Path(path) = &*input.func
        && let Some(name) = path.path.get_ident()
    {
        let name_str = name.to_string();
        match name_str.as_str() {
            // Type conversions: `int(x)` → `(x as i32)`, etc.
            "int" | "int8" | "int16" | "int32" | "int64"
            | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr" => {
                let rust_cast = match name_str.as_str() {
                    "int" => "i32",
                    "int8" => "i8",
                    "int16" => "i16",
                    "int32" => "i32",
                    "int64" => "i64",
                    "uint" => "u32",
                    "uint8" => "u8",
                    "uint16" => "u16",
                    "uint32" => "u32",
                    "uint64" => "u64",
                    "uintptr" => "usize",
                    _ => unreachable!(),
                };
                return quote! { (#(#args),* as #rust_cast) };
            }
            "float32" => {
                return quote! { (#(#args),* as f32) };
            }
            "float64" => {
                return quote! { (#(#args),* as f64) };
            }
            "bool" => {
                return quote! { (#(#args),* as bool) };
            }
            "string" => {
                let arg = args[0].clone();
                // string(bytes) → from_utf8(...)
                // string(rune) → String::from(char to string)
                return quote! { std::str::from_utf8(&#arg).unwrap_or("").to_string() };
            }
            "byte" => {
                return quote! { (#(#args),* as u8) };
            }
            "rune" => {
                return quote! { (#(#args),* as char) };
            }
            _ => {}
        }
    }
    let func = go_to_rust(&input.func);
    quote! { #func( #(#args),* ) }
}

fn transpile_paren(input: &syn::ExprParen) -> TokenStream {
    let inner = go_to_rust(&input.expr);
    quote! { ( #inner ) }
}

fn transpile_index(input: &ExprIndex) -> TokenStream {
    let seq = go_to_rust(&input.expr);
    let idx = go_to_rust(&input.index);
    quote! { #seq[ #idx ] }
}

fn transpile_method_call(input: &ExprMethodCall) -> TokenStream {
    let receiver = go_to_rust(&input.receiver);
    let method_name = &input.method;
    let args: Vec<_> = input.args.iter().map(go_to_rust).collect();
    if method_name.to_string() == "get" {
        if let Some(first) = args.first() {
            let rest: Vec<_> = args.iter().skip(1).cloned().collect();
            return quote! { #receiver.#method_name( &#first #(#rest),* ) };
        }
    }
    quote! { #receiver.#method_name( #(#args),* ) }
}

fn transpile_field(input: &ExprField) -> TokenStream {
    let base = go_to_rust(&input.base);
    let field = &input.member;
    quote! { #base.#field }
}

fn transpile_loop(input: &ExprLoop) -> TokenStream {
    let label = input.label.as_ref().map(|l| quote! { #l });
    let body = &input.body;
    quote! { loop #label #body }
}

fn transpile_for_loop(input: &ExprForLoop) -> TokenStream {
    let pat = &input.pat;
    let expr = go_to_rust(&input.expr);
    let body = &input.body;
    quote! { for #pat in #expr #body }
}

fn transpile_while(input: &ExprWhile) -> TokenStream {
    let label = input.label.as_ref().map(|l| quote! { #l });
    let cond = go_to_rust(&input.cond);
    let body = &input.body;
    quote! { while #cond #label #body }
}

fn transpile_range(input: &ExprRange) -> TokenStream {
    let _start = input.start.as_ref().map(|e| go_to_rust(e));
    let end = input.end.as_ref().map(|e| go_to_rust(e));
    let limits = match input.limits {
        syn::RangeLimits::HalfOpen(_) => quote! { .. },
        syn::RangeLimits::Closed(_)   => quote! { ..= },
    };
    match (input.start.as_ref(), input.end.as_ref()) {
        (Some(fd), Some(_ld))  => quote! { #fd #limits #end },
        (Some(e), None)        => quote! { #e #limits },
        (None, Some(e))        => quote! { #limits #e },
        (None, None)           => quote! { #limits },
    }
}

fn transpile_if(input: &ExprIf) -> TokenStream {
    let cond = go_to_rust(&input.cond);
    let then_block = &input.then_branch;
    let else_block = input.else_branch.as_ref().map(|(_, e)| {
        let e = go_to_rust(e);
        quote! { else { #e } }
    });
    quote! { if #cond #then_block #else_block }
}

fn transpile_block(input: &ExprBlock) -> TokenStream {
    if input.block.stmts.is_empty() {
        return quote! {{ }};
    }
    let mut outputs = Vec::new();
    for stm in input.block.stmts.iter() {
        match stm {
            syn::Stmt::Expr(val_expr, _) => {
                outputs.push(go_to_rust(val_expr));
            }
            syn::Stmt::Local(local) => {
                let local_pat = &local.pat;
                let local_val = local.init.as_ref().map(|v| go_to_rust(&v.expr));
                outputs.push(quote! { let #local_pat = #local_val; });
            }
            _ => return emit_todo("statement not yet supported"),
        }
    }
    quote! {{ { #(#outputs);* } }}
}

fn transpile_array(input: &ExprArray) -> TokenStream {
    let elems: Vec<_> = input.elems.iter().map(go_to_rust).collect();
    if elems.is_empty() {
        // In Go slice literals like `[]int{ 1, 2, 3 }`, syn parses `[]`
        // as an empty array expression. If the array has no elements,
        // this is likely the start of a Go slice literal. The actual
        // slice elements come from the `Expr::Verbatim` handling below.
        quote! { vec![] }
    } else {
        quote! { [#(#elems),*] }
    }
}

/// Handle `Expr::Verbatim` tokens produced by syn when it can't fully
/// parse Go slice/map literals. Looks for a brace group inside the
/// verbatim tokens, extracts elements, and emits `vec![...]`.
fn transpile_verbatim(tokens: &proc_macro2::TokenStream) -> TokenStream {
    use proc_macro2::TokenTree;

    for tt in tokens.clone() {
        if let TokenTree::Group(g) = tt
            && g.delimiter() == proc_macro2::Delimiter::Brace
        {
            let brace_content = g.stream();

            // Parse elements from the brace group using a custom parser
            #[derive(Default)]
            struct ElemParser { elems: Vec<Expr>, }
            impl Parse for ElemParser {
                fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
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
            let elems: Vec<_> = parser.elems.iter().map(|expr| go_to_rust(expr)).collect();
            return quote! { vec![ #(#elems),* ] };
        }
    }

    // No brace group — emit raw tokens (simple literals)
    quote! { #tokens }
}
