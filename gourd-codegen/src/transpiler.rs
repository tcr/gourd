use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, BinOp, ExprBlock, ExprIf, UnOp};

/// Emit a compile-time error for forms we don't support.
fn emit_todo(msg: &'static str) -> TokenStream {
    quote! { {  compile_error!( concat!("TODO: ", #msg) );  unreachable!() } }
}

/// Dispatch the AST per expression node
pub fn go_to_rust(input: &Expr) -> TokenStream {
    match input {
        Expr::Lit(e)      => transpile_lit(e),
        Expr::Binary(e)   => transpile_binary(e),
        Expr::Unary(e)    => transpile_unary(e),
        Expr::Path(e)     => transpile_path(e),
        Expr::Call(e)     => transpile_call(e),
        Expr::Paren(e)    => transpile_paren(e),
        Expr::Group(e)          => go_to_rust(&e.expr),
        Expr::If(e)             => transpile_if(e),
        Expr::Block(e)    => transpile_block(e),
        // Go = Rust: let
        Expr::Let(e)        => transpile_let(e),
        // Unsupported
        _                   => emit_todo("unsupported Go form"),
    }
}

/// Literals pass through directly.
fn transpile_lit(input: &syn::ExprLit) -> TokenStream {
    let lit = &input.lit;
    quote! {  #lit }
}

/// Go path `nil` → Rust `Option::None`, `true`/`false` → Rust `true`/`false`.
fn transpile_path(input: &syn::ExprPath) -> TokenStream {
    let p = &input.path;
    match p.get_ident() {
        Some(ident) => match ident.to_string().as_str() {
            "nil"  => quote! {  None },
            "true" => quote! {  true },
            "false" => quote! {  false },
            _ => quote! {  #p },
        },
        None => quote! {  #p },
    }
}

fn transpile_binary(input: &syn::ExprBinary) -> TokenStream {
    let lhs = go_to_rust(&input.left);
    let rhs = go_to_rust(&input.right);
    match input.op {
        BinOp::Add(_)     => quote! {  #lhs + #rhs  },
        BinOp::Sub(_)     => quote! {  #lhs - #rhs  },
        BinOp::Mul(_) => quote! {  #lhs * #rhs  },
        BinOp::Div(_) => quote! {  #lhs / #rhs  },
        BinOp::Rem(_) => quote! {  #lhs % #rhs  },
        BinOp::And(_) => quote! {  #lhs && #rhs  },
        BinOp::Or(_)  => quote! {  #lhs || #rhs  },
        BinOp::BitXor(_)  => quote! {  #lhs ^ #rhs  },
        BinOp::BitAnd(_)  => quote! {  #lhs & #rhs  },
        BinOp::BitOr(_)   => quote! {  #lhs | #rhs  },
        BinOp::Shl(_)     => quote! {  #lhs << #rhs  },
        BinOp::Shr(_)     => quote! {  #lhs >> #rhs  },
        BinOp::Eq(_)      => quote! {  #lhs == #rhs  },
        BinOp::Ne(_)      => quote! {  #lhs != #rhs  },
        BinOp::Ge(_)      => quote! {  #lhs >= #rhs  },
        BinOp::Gt(_)      => quote! {  #lhs >  #rhs  },
        BinOp::Le(_)      => quote! {  #lhs <= #rhs  },
        BinOp::Lt(_)      => quote! {  #lhs <  #rhs  },
        _ => emit_todo("unsupported binary operator"),
    }
}

fn transpile_unary(input: &syn::ExprUnary) -> TokenStream {
    let inner = go_to_rust(&input.expr);
    match &input.op {
        UnOp::Not(_)   => quote! {  ! #inner },
        UnOp::Neg(_)   => quote! {  - #inner },
        UnOp::Deref(_)  => quote! {  * #inner },
        _               => emit_todo("unsupported unary operator"),
    }
}

/// Go `x := y` = Rust `let x = y`
fn transpile_let(input: &syn::ExprLet) -> TokenStream {
    let pat = &input.pat;
    let expr = go_to_rust(&input.expr);
    quote! {  let #pat  = #expr }
}

/// Go `len(slice)` or `cap(slice)` → Rust `slice.len()`
fn transpile_call(input: &syn::ExprCall) -> TokenStream {
    let args: Vec<_> = input.args.iter().map(go_to_rust).collect();
    if let Expr::Path(path) = &*input.func {
        if let Some(name) = path.path.get_ident() {
            let n = name.to_string();
            if n == "len" || n == "cap" {
                let arg = args[0].clone();
                return quote! {  #arg.len() };
            }
        }
    }
    let func = go_to_rust(&input.func);
    quote! {  #func( #(#args),* ) }
}

/// `(x)  →  (x)`
fn transpile_paren(input: &syn::ExprParen) -> TokenStream {
    let inner = go_to_rust(&input.expr);
    quote! { ( #inner ) }
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

/// An `{ ... }` block: transpile each statement; the final expression
/// becomes the block's value.
fn transpile_block(input: &ExprBlock) -> TokenStream {
    if input.block.stmts.is_empty() {
        return quote! { { } };
    }
    let mut outputs = Vec::new();
    for stm in input.block.stmts.iter() {
        match stm {
            syn::Stmt::Expr(val_expr, _semicolon)  => {
                outputs.push(go_to_rust(val_expr));
            }
            syn::Stmt::Local(local)          => {
                let local_pat = &local.pat;
                let local_val = local.init.as_ref().map(|v| go_to_rust(&v.expr));
                outputs.push(quote! { let #local_pat = #local_val; });
            }
            _                                => {
                return emit_todo("statement not yet supported");
            }
        }
    }
    quote! { { #(#outputs);* } }
}
