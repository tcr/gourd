//! Go AST → HIR conversion.
//!
//! This module converts the existing Go AST (`ast.rs`) into the HIR
//! representation. The conversion happens in layers:
//!
//! 1. **Expressions** (leaf nodes) — go_ast_expr_to_hir
//! 2. **Statements** — go_ast_stmt_to_hir
//! 3. **Control flow** — go_ast_control_to_hir
//! 4. **Declarations** — go_ast_decl_to_hir

use syn::{Expr, ExprIf, ExprRange, ExprLoop, ExprForLoop, ExprWhile, ExprLet, ExprBreak, ExprContinue, ExprReturn, ExprReference, ExprClosure, ExprArray, ExprCast, ExprAssign, ExprCall, ExprMethodCall, ExprField, ExprIndex, ExprParen, ExprTuple, ExprLit, ExprBinary, ExprUnary, ExprPath, Pat, PatIdent, Ident, Type, BinOp, Token};
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use super::expression::*;
use super::types::primitives::{HirType, HirTypeKind};
use super::types::mapping::{go_type_to_hir};
use super::types::compound::{HirSelect, HirSelectCase, HirSwitch, HirSwitchCase};
use super::statement::*;
// Import HIR module types for all conversion functions
use super::ast::{GoBlock, GoStmt, GoSelect, GoSelectCase, Switch, GoImport};

/// Convert a Go `syn::Expr` to a HIR expression.
///
/// This is the core conversion function. It walks the `syn::Expr` tree
/// and converts each node to the corresponding HIR node.
/// Check if a HIR expression is a simple identifier.
pub fn is_simple_identifier(expr: &HirExpr) -> bool {
    matches!(&expr.kind, HirExprKind::Identifier(_))
}

/// Get the identifier name if this is an identifier expression.
pub fn get_identifier_name(expr: &HirExpr) -> Option<&syn::Ident> {
    match &expr.kind {
        HirExprKind::Identifier(id) => Some(id),
        _ => None,
    }
}


/// Preprocess Go slice literal syntax `[]T{elems}` in a raw TokenStream
/// to Rust `vec![elems]` syntax so that `syn::parse2::<Expr>` can handle it.
fn preprocess_go_slice_literals(ts: TokenStream) -> TokenStream {
    use proc_macro2::{TokenTree, Group, Punct, Spacing, Delimiter};
    let mut result = Vec::new();
    let trees: Vec<TokenTree> = ts.into_iter().collect();
    let mut i = 0;

    while i < trees.len() {
        match &trees[i] {
            TokenTree::Punct(p) if p.as_char() == '[' => {
                // Check for Go slice literal: []T{elems}
                let rest = &trees[i + 1..];
                if rest.len() >= 3 {
                    if matches!(&rest[0], TokenTree::Punct(pp) if pp.as_char() == ']')
                        && matches!(&rest[1], TokenTree::Ident(_))
                        && matches!(&rest[2], TokenTree::Group(g) if g.delimiter() == Delimiter::Brace)
                    {
                        // Emit `GoSlice::from(vec![elems])` instead of `[]T{...}`
                        let inner = rest[2].clone();
                        if let TokenTree::Group(g) = &inner {
                            result.push(TokenTree::Ident(syn::Ident::new("GoSlice", proc_macro2::Span::call_site())));
                            result.push(TokenTree::Punct(Punct::new(':', Spacing::Joint)));
                            result.push(TokenTree::Punct(Punct::new(':', Spacing::Alone)));
                            result.push(TokenTree::Ident(syn::Ident::new("from", proc_macro2::Span::call_site())));
                            result.push(TokenTree::Group(Group::new(
                                Delimiter::Parenthesis,
                                TokenStream::from_iter(vec![
                                    TokenTree::Ident(syn::Ident::new("vec", proc_macro2::Span::call_site())),
                                    TokenTree::Punct(Punct::new('!', Spacing::Joint)),
                                    inner,
                                ]),
                            )));
                            i += 3;
                        } else {
                            result.push(trees[i].clone());
                            i += 1;
                        }
                    } else {
                        result.push(trees[i].clone());
                        i += 1;
                    }
                } else {
                    result.push(trees[i].clone());
                    i += 1;
                }
            }
            TokenTree::Ident(ident) if ident == "map" => {
                // Check for Go map literal: map<K,V>{entries} or map[K]V{entries}
                if i + 1 < trees.len() {
                    let rest = &trees[i + 1..];
                    // Look for either `[` (Go-style K]V{}) or `<` (generic style)
                    let start_char = if let Some(pos) = rest.iter().position(|t| {
                        matches!(t, TokenTree::Punct(p) if p.as_char() == '[')
                    }) {
                        Some((pos, '[', ']'))
                    } else if let Some(pos) = rest.iter().position(|t| {
                        matches!(t, TokenTree::Punct(p) if p.as_char() == '<')
                    }) {
                        Some((pos, '<', '>'))
                    } else {
                        None
                    };

                    if let Some((punct_pos, open_char, close_char)) = start_char {
                        let after_map = &rest[punct_pos..];
                        if let Some(end_pos) = after_map.iter().position(|t| {
                            matches!(t, TokenTree::Punct(p) if p.as_char() == close_char)
                        }) {
                            // After `]` or `>`, look for `{...}` body
                            let after_bracket = &after_map[end_pos + 1..];
                            if let Some(brace_pos) = after_bracket.iter().position(|t| {
                                matches!(t, TokenTree::Group(g) if g.delimiter() == Delimiter::Brace)
                            }) {
                                // For square bracket style map[K]V, we need to parse K and V separately
                                // because `string]int` is not valid Rust type syntax
                                let is_bracket_style = open_char == '[';

                                // Push `std :: collections :: HashMap`
                                for part in ["std", "collections", "HashMap"] {
                                    result.push(TokenTree::Ident(syn::Ident::new(part, proc_macro2::Span::call_site())));
                                    result.push(TokenTree::Punct(Punct::new(':', Spacing::Joint)));
                                }

                                // Extract K and V from the bracket content
                                if is_bracket_style {
                                    // Square bracket style: map[K]V -> parse K from [K], V follows after ]
                                    let bracket_content = &after_map[1..end_pos]; // e.g. [string]
                                    let key_str = bracket_content.iter().map(|t| quote! { #t }.to_string()).collect::<String>().trim().to_string();
                                    let val_content = &after_bracket[..brace_pos]; // type after ]
                                    let val_str = val_content.iter().map(|t| quote! { #t }.to_string()).collect::<String>().trim().to_string();
                                    let key: Option<syn::Type> = syn::parse_str(&key_str).ok();
                                    let val: Option<syn::Type> = syn::parse_str(&val_str).ok();
                                    let gt_inner = quote! { < #key , #val > };
                                    result.push(TokenTree::Group(Group::new(Delimiter::None, gt_inner)));
                                } else {
                                    // Angle bracket style: map<K,V>
                                    let key_val_ts: TokenStream = after_map[1..end_pos].iter().cloned().collect();
                                    let key_type: Option<syn::Type> = syn::parse2(key_val_ts).ok();
                                    if let Some(key_type) = &key_type {
                                        let inner_str = quote! { #key_type }.to_string();
                                        if let Some(comma_pos) = inner_str.find(',') {
                                            let key_str = inner_str[..comma_pos].trim().to_string();
                                            let val_str = inner_str[comma_pos+1..].trim().to_string();
                                            let key: Option<syn::Type> = syn::parse_str(&key_str).ok();
                                            let val: Option<syn::Type> = syn::parse_str(&val_str).ok();
                                            let gt_inner = quote! { < #key , #val > };
                                            result.push(TokenTree::Group(Group::new(Delimiter::None, gt_inner)));
                                        } else {
                                            let key = key_type.clone();
                                            let val: syn::Type = syn::parse_str("String").unwrap();
                                            let gt_inner = quote! { < #key , #val > };
                                            result.push(TokenTree::Group(Group::new(Delimiter::None, gt_inner)));
                                        }
                                    } else {
                                        result.push(TokenTree::Group(Group::new(Delimiter::None, quote! { < i32 , String > })));
                                    }
                                }

                                // Push ` :: default()`
                                result.push(TokenTree::Punct(Punct::new(':', Spacing::Joint)));
                                result.push(TokenTree::Punct(Punct::new(':', Spacing::Alone)));
                                result.push(TokenTree::Ident(syn::Ident::new("default", proc_macro2::Span::call_site())));
                                result.push(TokenTree::Punct(Punct::new('(', Spacing::Joint)));
                                result.push(TokenTree::Punct(Punct::new(')', Spacing::Alone)));
                                i += 1;
                            }
                        }
                    }
                } else {
                    result.push(trees[i].clone());
                    i += 1;
                }
            }
            _ => {
                result.push(trees[i].clone());
                i += 1;
            }
        }
    }

    result.into_iter().collect()
}

pub fn go_ast_expr_to_hir(expr: &Expr) -> HirExpr {
    match expr {
        Expr::Lit(lit) => hir_lit_to_hir(lit),
        Expr::Path(path) => hir_path_to_hir(path),
        Expr::Binary(binary) => hir_binary_to_hir(binary),
        Expr::Unary(unary) => hir_unary_to_hir(unary),
        Expr::Call(call) => hir_call_to_hir(call),
        Expr::MethodCall(method) => hir_method_call_to_hir(method),
        Expr::Field(field) => hir_field_to_hir(field),
        Expr::Index(index) => hir_index_to_hir(index),
        Expr::Paren(paren) => hir_paren_to_hir(paren),
        Expr::Array(array) => hir_array_to_hir(array),
        Expr::Cast(cast) => hir_cast_to_hir(cast),
        Expr::Assign(assign) => hir_assign_to_hir(assign),
        Expr::If(if_expr) => hir_if_to_hir(if_expr),
        Expr::Range(range) => hir_range_to_hir(range),
        Expr::Loop(loop_expr) => hir_loop_to_hir(loop_expr),
        Expr::ForLoop(for_loop) => hir_for_loop_to_hir(for_loop),
        Expr::While(while_expr) => hir_while_to_hir(while_expr),
        Expr::Let(let_expr) => hir_let_to_hir(let_expr),
        Expr::Tuple(tuple) => hir_tuple_to_hir(tuple),
        Expr::Break(break_expr) => hir_break_to_hir(break_expr),
        Expr::Continue(cont) => hir_continue_to_hir(cont),
        Expr::Return(ret) => hir_return_to_hir(ret),
        Expr::Group(group) => hir_group_to_hir(group),
        Expr::Reference(reference) => hir_reference_to_hir(reference),
        Expr::Closure(closure) => hir_closure_to_hir_expr(closure),
        Expr::Struct(struct_expr) => hir_struct_to_hir(struct_expr),
        Expr::Macro(macro_expr) => hir_macro_to_hir(macro_expr),
        Expr::Match(match_expr) => hir_match_to_hir(match_expr),
        Expr::Verbatim(tokens) => HirExpr::new(HirExprKind::Macro(tokens.clone())),
        Expr::Yield(yield_expr) => {
            // Yield expressions: `^expr` — pass through as unary not
            let inner = Box::new(go_ast_expr_to_hir(&**yield_expr.expr.as_ref().unwrap()));
            HirExpr::new(HirExprKind::Unary {
                op: HirUnaryOp::Not,
                operand: inner,
            })
        }
        _ => HirExpr::new(HirExprKind::Unsupported("unsupported Go expression".to_string())),
    }
}

/// Convert a Go literal to HIR.
fn hir_lit_to_hir(lit: &ExprLit) -> HirExpr {
    let lit = &lit.lit;
    match lit {
        syn::Lit::Str(s) => HirExpr::new(HirExprKind::Literal(HirLiteral::StringTy(s.value()))),
        syn::Lit::Int(n) => {
            // Go integers default to i32 semantics.
            // Store as u64 so quote! produces clean integers without suffix.
            HirExpr::new(HirExprKind::Literal(HirLiteral::Int(n.base10_parse::<u64>().unwrap_or(0))))
        }
        syn::Lit::Float(f) => HirExpr::new(HirExprKind::Literal(HirLiteral::Float(f.base10_parse().unwrap_or(0.0)))),
        syn::Lit::Bool(b) => HirExpr::new(HirExprKind::Literal(HirLiteral::Bool(b.value))),
        syn::Lit::Byte(b) => HirExpr::new(HirExprKind::Literal(HirLiteral::Int(u64::from(b.value())))),
        syn::Lit::ByteStr(bs) => {
            // Byte string: `b"..."` — convert to Vec<u8>
            let bytes: Vec<HirExpr> = bs.value().iter().map(|&b| {
                HirExpr::new(HirExprKind::Literal(HirLiteral::Int(u64::from(b))))
            }).collect();
            HirExpr::new(HirExprKind::SliceLiteral(bytes))
        }
        syn::Lit::Char(c) => {
            // Character literal: `'x'` in Go is a rune (int32 = Unicode code point).
            // Treat as integer for arithmetic: `'0'` → 48, `'a'` → 97, etc.
            HirExpr::new(HirExprKind::Literal(HirLiteral::Int(c.value() as u64)))
        }
        _ => HirExpr::new(HirExprKind::Unsupported("unknown literal".to_string())),
    }
}

/// Convert a Go path to HIR.
fn hir_path_to_hir(path: &syn::ExprPath) -> HirExpr {
    if path.path.is_ident("nil") {
        HirExpr::new(HirExprKind::Literal(HirLiteral::Nil))
    } else if path.path.is_ident("true") {
        HirExpr::new(HirExprKind::Literal(HirLiteral::Bool(true)))
    } else if path.path.is_ident("false") {
        HirExpr::new(HirExprKind::Literal(HirLiteral::Bool(false)))
    } else if path.path.segments.len() == 1 {
        let ident = path.path.segments.first().unwrap().ident.clone();
        HirExpr::new(HirExprKind::Identifier(ident))
    } else {
        // Full path like `gourd::prelude::fields` or `::std::string::String::from`
        use super::expression::HirPath;
        HirExpr::new(HirExprKind::Path(HirPath(path.path.clone())))
    }
}

/// Convert a Go binary expression to HIR.
fn hir_binary_to_hir(binary: &ExprBinary) -> HirExpr {
    // Detect channel send: Go `ch <- value` is tokenized by syn as
    // `<` (binary less-than) with `-value` (unary negation) on the right.
    // We need to special-case this so it produces ChannelSend instead of a
    // comparison.
    if matches!(binary.op, BinOp::Lt(_)) {
        if let Expr::Unary(ExprUnary { op: syn::UnOp::Neg(_), expr, .. }) = binary.right.as_ref() {
            let channel = Box::new(go_ast_expr_to_hir(&binary.left));
            let value = go_ast_expr_to_hir(expr);
            return HirExpr::new(HirExprKind::ChannelSend {
                channel,
                value: Box::new(value),
            });
        }
    }

    // Detect nil comparisons: Go `m == nil` on maps/channels
    // Must check BEFORE extracting lhs/rhs to short-circuit early
    if matches!(binary.op, BinOp::Eq(_) | BinOp::Ne(_)) {
        // Parse both sides first to check for nil on rhs
        let lhs_expr = go_ast_expr_to_hir(&binary.left);
        let rhs_expr = go_ast_expr_to_hir(&binary.right);
        
        // Check if rhs is the nil literal
        if let HirExprKind::Literal(HirLiteral::Nil) = rhs_expr.kind {
            let negative = matches!(binary.op, BinOp::Ne(_));
            return HirExpr::new(HirExprKind::NilComparison {
                collection: Box::new(lhs_expr),
                negative,
            });
        }
        
        let op = hir_binary_op_to_hir(&binary.op);
        let lhs = Box::new(lhs_expr);
        let rhs = Box::new(rhs_expr);
        HirExpr::new(HirExprKind::Binary { op, lhs, rhs })
    } else {
        let op = hir_binary_op_to_hir(&binary.op);
        let lhs = Box::new(go_ast_expr_to_hir(&binary.left));
        let rhs = Box::new(go_ast_expr_to_hir(&binary.right));
        HirExpr::new(HirExprKind::Binary { op, lhs, rhs })
    }
}

/// Convert a Go binary operator to HIR.
fn hir_binary_op_to_hir(op: &BinOp) -> HirBinaryOp {
    match op {
        BinOp::Add(_) => HirBinaryOp::Add,
        BinOp::Sub(_) => HirBinaryOp::Sub,
        BinOp::Mul(_) => HirBinaryOp::Mul,
        BinOp::Div(_) => HirBinaryOp::Div,
        BinOp::Rem(_) => HirBinaryOp::Mod,
        BinOp::Eq(_) => HirBinaryOp::Eq,
        BinOp::Ne(_) => HirBinaryOp::Ne,
        BinOp::Lt(_) => HirBinaryOp::Lt,
        BinOp::Le(_) => HirBinaryOp::Le,
        BinOp::Gt(_) => HirBinaryOp::Gt,
        BinOp::Ge(_) => HirBinaryOp::Ge,
        BinOp::And(_) => HirBinaryOp::And,
        BinOp::Or(_) => HirBinaryOp::Or,
        BinOp::BitAnd(_) => HirBinaryOp::BitAnd,
        BinOp::BitOr(_) => HirBinaryOp::BitOr,
        BinOp::BitXor(_) => HirBinaryOp::BitXor,
        BinOp::Shl(_) => HirBinaryOp::Lsh,
        BinOp::Shr(_) => HirBinaryOp::Rsh,
        // Compound assignment operators (from `i += 1` style expressions)
        BinOp::AddAssign(_) => HirBinaryOp::AddAssign,
        BinOp::SubAssign(_) => HirBinaryOp::SubAssign,
        BinOp::MulAssign(_) => HirBinaryOp::MulAssign,
        BinOp::DivAssign(_) => HirBinaryOp::DivAssign,
        BinOp::RemAssign(_) => HirBinaryOp::ModAssign,
        BinOp::BitAndAssign(_) => HirBinaryOp::AndAssign,
        BinOp::BitOrAssign(_) => HirBinaryOp::OrAssign,
        BinOp::BitXorAssign(_) => HirBinaryOp::XorAssign,
        BinOp::ShlAssign(_) => HirBinaryOp::LshAssign,
        BinOp::ShrAssign(_) => HirBinaryOp::RshAssign,
        _ => HirBinaryOp::Add, // fallback
    }
}

/// Convert a Go unary expression to HIR.
fn hir_unary_to_hir(unary: &ExprUnary) -> HirExpr {
    let operand = Box::new(go_ast_expr_to_hir(&unary.expr));
    let op = hir_unary_op_to_hir(&unary.op);
    HirExpr::new(HirExprKind::Unary { op, operand })
}

/// Convert a Go unary operator to HIR.
fn hir_unary_op_to_hir(op: &syn::UnOp) -> HirUnaryOp {
    match op {
        syn::UnOp::Not(_) => HirUnaryOp::Not,
        syn::UnOp::Neg(_) => HirUnaryOp::Neg,
        syn::UnOp::Deref(_) => HirUnaryOp::Deref,
        _ => HirUnaryOp::Not, // fallback
    }
}

/// Convert a Go call expression to HIR.
fn hir_call_to_hir(call: &ExprCall) -> HirExpr {
    // Check for known builtin function calls
    if let Expr::Path(path) = &*call.func {
        // Handle std::copy, std::delete, std::append
        if path.path.segments.len() == 2 {
            let pkg = path.path.segments[0].ident.to_string();
            if pkg == "std" {
                let func = path.path.segments[1].ident.to_string();
                if matches!(func.as_str(), "copy" | "delete" | "append") {
                    let args: Vec<HirExpr> = call.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
                    return HirExpr::new(HirExprKind::StdCall { func_name: func, args });
                }
            }
        }
        // Handle `new(Foo)` → `Foo::default()`
        if let Some(name) = path.path.get_ident()
            && name.to_string() == "new"
        {
            if call.args.len() == 1 {
                let arg = &call.args[0];
                // For type names (paths), map Go type → Rust type and emit ::default()
                if let Expr::Path(arg_path) = arg {
                    let type_str = quote! { #arg_path }.to_string();
                    // Map Go primitive type names to Rust equivalents
                    let mapped_str = match type_str.as_str() {
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
                        "byte" => "u8",
                        "rune" => "char",
                        "float32" => "f32",
                        "float64" => "f64",
                        "string" => "String",
                        "bool" => "bool",
                        "error" => "Box<dyn std::error::Error>",
                        _ => &type_str,
                    };
                    if let Ok(mapped_ty) = syn::parse_str::<syn::Type>(mapped_str) {
                        if let syn::Type::Path(type_path) = mapped_ty {
                            return HirExpr::new(HirExprKind::New(Box::new(HirExpr::new(HirExprKind::Path(super::expression::HirPath(type_path.path))))));
                        }
                    }
                }
                // For user-defined types, emit ::default()
                let arg_hir = go_ast_expr_to_hir(arg);
                return HirExpr::new(HirExprKind::New(Box::new(arg_hir)));
            }
        }
        if let Some(name) = path.path.get_ident() {
            let name_str = name.to_string();
            // Go type conversions: int(), string(), bool(), etc.
            if matches!(
                name_str.as_str(),
                "int" | "int8" | "int16" | "int32" | "int64"
                | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr"
                | "float32" | "float64" | "bool" | "byte" | "rune"
            ) {
                if let Some(arg) = call.args.first() {
                    let arg_expr = go_ast_expr_to_hir(arg);
                    return HirExpr::new(HirExprKind::TypeConvert { func: name.clone(), arg: Box::new(arg_expr) });
                }
            }
            // String conversion (special: bytes → String)
            if name_str == "string" {
                if let Some(arg) = call.args.first() {
                    let arg_expr = go_ast_expr_to_hir(arg);
                    return HirExpr::new(HirExprKind::TypeConvert { func: name.clone(), arg: Box::new(arg_expr) });
                }
            }
            // len(s) → s.len() as i32
            if name_str == "len" {
                if let Some(arg) = call.args.first() {
                    let arg_expr = go_ast_expr_to_hir(arg);
                    return HirExpr::new(HirExprKind::Len(Box::new(arg_expr)));
                }
            }
            // cap(s) → s.capacity() as i32
            if name_str == "cap" {
                if let Some(arg) = call.args.first() {
                    let arg_expr = go_ast_expr_to_hir(arg);
                    return HirExpr::new(HirExprKind::Cap(Box::new(arg_expr)));
                }
            }
            // min(a, b) → min(a, b) via runtime
            if name_str == "min" {
                if call.args.len() >= 2 {
                    let args: Vec<HirExpr> = call.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
                    let args_arc: Vec<Box<HirExpr>> = args.into_iter().map(Box::new).collect();
                    return HirExpr::new(HirExprKind::MinMax { kind: "min".to_string(), values: args_arc });
                }
            }
            // max(a, b) → max(a, b) via runtime
            if name_str == "max" {
                if call.args.len() >= 2 {
                    let args: Vec<HirExpr> = call.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
                    let args_arc: Vec<Box<HirExpr>> = args.into_iter().map(Box::new).collect();
                    return HirExpr::new(HirExprKind::MinMax { kind: "max".to_string(), values: args_arc });
                }
            }
            // panic("msg") → panic!("msg")
            // panic() → panic!("panic()")
            if name_str == "panic" {
                if let Some(arg) = call.args.first() {
                    if let HirExprKind::Literal(HirLiteral::StringTy(msg)) = &go_ast_expr_to_hir(arg).kind {
                        return HirExpr::new(HirExprKind::Panic(msg.clone()));
                    }
                } else {
                    return HirExpr::new(HirExprKind::Panic("panic()".to_string()));
                }
            }
            // make(type, len) or make(type, len, cap) → Vec/HashMap/Channel creation
            if name_str == "make" {
                if call.args.len() >= 2 {
                    let _args_hir: Vec<HirExpr> = call.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
                    let len_expr = Box::new(go_ast_expr_to_hir(&call.args[1]));
                    let cap_expr: Option<Box<HirExpr>> = if call.args.len() > 2 {
                        Some(Box::new(go_ast_expr_to_hir(&call.args[2])))
                    } else {
                        None
                    };
                    // Parse type string to HirType
                    let type_str = quote::quote! { #call.args[0] }.to_string();
                    let hir_type = super::types::parse_go_type(&type_str);
                    if type_str.starts_with("map[") {
                        // Extract key and value types from "map[K]V"
                        let map_content = type_str.strip_prefix("map[").unwrap_or("");
                        let bracket_pos = map_content.find(']').unwrap_or(map_content.len());
                        let key_str = &map_content[..bracket_pos];
                        let val_str = if bracket_pos < map_content.len() {
                            &map_content[bracket_pos + 1..]
                        } else {
                            "unknown"
                        };
                        let key_hir = super::types::parse_go_type(key_str.trim());
                        let val_hir = super::types::parse_go_type(val_str.trim());
                        if let Some(cap) = cap_expr {
                            return HirExpr::new(HirExprKind::Make(
                                super::expression::MakeKind::MapWithCap(Box::new(key_hir), Box::new(val_hir), cap)
                            ));
                        } else {
                            return HirExpr::new(HirExprKind::Make(
                                super::expression::MakeKind::Map(Box::new(key_hir), Box::new(val_hir))
                            ));
                        }
                    } else if type_str.starts_with("chan") || type_str.contains("chan") {
                        if let Some(cap) = cap_expr {
                            return HirExpr::new(HirExprKind::Make(
                                super::expression::MakeKind::ChannelWithCap(Box::new(hir_type), cap)
                            ));
                        } else {
                            return HirExpr::new(HirExprKind::Make(
                                super::expression::MakeKind::Channel(Box::new(hir_type))
                            ));
                        }
                    } else {
                        if let Some(cap) = cap_expr {
                            return HirExpr::new(HirExprKind::Make(
                                super::expression::MakeKind::SliceWithCap(Box::new(hir_type), len_expr, cap)
                            ));
                        } else {
                            return HirExpr::new(HirExprKind::Make(
                                super::expression::MakeKind::Slice(Box::new(hir_type), len_expr)
                            ));
                        }
                    }
                }
            }
            // append(slice, items...) → push to Vec
            if name_str == "append" {
                if !call.args.is_empty() {
                    let target = Box::new(go_ast_expr_to_hir(&call.args.first().unwrap()));
                    let elements: Vec<HirExpr> = call.args.iter().skip(1).map(|a| go_ast_expr_to_hir(a)).collect();
                    return HirExpr::new(HirExprKind::Append { target, elements });
                }
            }
            // delete(map, key) → HashMap::remove
            if name_str == "delete" {
                if call.args.len() >= 2 {
                    let map_expr = Box::new(go_ast_expr_to_hir(&call.args[0]));
                    let key_expr = Box::new(go_ast_expr_to_hir(&call.args[1]));
                    return HirExpr::new(HirExprKind::Delete { map: map_expr, key: key_expr });
                }
            }
        }
        // Check for strings.Fields(path) → field access pattern
        if path.path.segments.len() == 2 {
            let seg0 = &path.path.segments[0].ident;
            let seg1 = &path.path.segments[1].ident;
            let seg0_s = seg0.to_string();
            let seg1_s = seg1.to_string();
            if seg0_s == "strings" && seg1_s == "Fields" {
                let args: Vec<HirExpr> = call.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
                return HirExpr::new(HirExprKind::Call {
                    func: Box::new(HirExpr::new(HirExprKind::Path(
                        super::expression::HirPath(path.path.clone())
                    ))),
                    args,
                });
            }
            if seg0_s == "strings" && seg1_s == "Join" {
                let args: Vec<HirExpr> = call.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
                return HirExpr::new(HirExprKind::Call {
                    func: Box::new(HirExpr::new(HirExprKind::Path(
                        super::expression::HirPath(path.path.clone())
                    ))),
                    args,
                });
            }
            if seg0_s == "fmt" && seg1_s == "Sprintf" {
                let args: Vec<HirExpr> = call.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
                return HirExpr::new(HirExprKind::Call {
                    func: Box::new(HirExpr::new(HirExprKind::Path(
                        super::expression::HirPath(path.path.clone())
                    ))),
                    args,
                });
            }
            // fmt.Println, fmt.Print, fmt.Printf → fmt_println/print/printf
            if seg0_s == "fmt" && matches!(seg1_s.as_str(), "Print" | "Println" | "Printf") {
                let seg1 = seg1_s.clone();
                let rust_fn = match seg1.as_str() {
                    "Print" => "fmt_print",
                    "Println" => "fmt_println",
                    "Printf" => "fmt_printf",
                    _ => unreachable!(),
                };
                let args: Vec<HirExpr> = call.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
                return HirExpr::new(HirExprKind::Call {
                    func: Box::new(HirExpr::new(HirExprKind::Path(
                        super::expression::HirPath(syn::parse_str(&format!("::gourd::prelude::{}", rust_fn)).unwrap()),
                    ))),
                    args,
                });
            }
        }
    }
    // Generic function call
    let func = Box::new(go_ast_expr_to_hir(&call.func));
    let args: Vec<HirExpr> = call.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
    HirExpr::new(HirExprKind::Call { func, args })
}

/// Convert a Go method call to HIR.
fn hir_method_call_to_hir(method: &ExprMethodCall) -> HirExpr {
    let method_name = method.method.to_string();
    let receiver_str = quote::quote!(#method.receiver).to_string();
    // Handle fmt.Println, fmt.Print, fmt.Printf → flat function calls
    if receiver_str.contains("fmt") && matches!(method_name.as_str(), "Print" | "Println" | "Printf") {
        let rust_fn = match method_name.as_str() {
            "Print" => "fmt_print",
            "Println" => "fmt_println",
            "Printf" => "fmt_printf",
            _ => unreachable!(),
        };
        let args: Vec<HirExpr> = method.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
        return HirExpr::new(HirExprKind::Call {
            func: Box::new(HirExpr::new(HirExprKind::Path(
                super::expression::HirPath(syn::parse_str(&format!("::gourd::prelude::{}", rust_fn)).unwrap()),
            ))),
            args,
        });
    }
    // Handle std::copy, std::delete, std::append method calls
    if receiver_str.contains("std") {
        let args: Vec<HirExpr> = method.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
        let func_name: &'static str = match method_name.as_str() {
            "copy" => "copy",
            "delete" => "delete",
            "append" => "append",
            _ => "SKIP"
        };
        if !func_name.is_empty() {
            return HirExpr::new(HirExprKind::StdCall { func_name: func_name.to_string(), args });
        }
    }

    // Handle fmt.Sprintf → flat function call
    if receiver_str.contains("fmt") && method_name == "Sprintf" {
        let args: Vec<HirExpr> = method.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
        return HirExpr::new(HirExprKind::Call {
            func: Box::new(HirExpr::new(HirExprKind::Path(
                super::expression::HirPath(syn::parse_str("::gourd::prelude::fmt_sprintf").unwrap()),
            ))),
            args,
        });
    }
    // Handle strings package method calls → flat function calls
    if receiver_str.contains("strings") {
        let args: Vec<HirExpr> = method.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
        let rust_fn: &'static str = match method_name.as_str() {
            "Replace" => "strings_replace",
            "ReplaceAll" => "strings_replace_all",
            "HasPrefix" => "has_prefix",
            "HasSuffix" => "has_suffix",
            "Contains" => "contains_str",
            "Split" => "split",
            "Join" => "join",
            "Index" => "index_str",
            "LastIndex" => "last_index_str",
            "Trim" => "trim",
            "TrimLeft" => "trim_left",
            "TrimRight" => "trim_right",
            "ToUpper" => "to_upper",
            "ToLower" => "to_lower",
            "Repeat" => "repeat",
            "Fields" => "fields",
            _ => "SKIP"
        };
        if !rust_fn.is_empty() {
            return HirExpr::new(HirExprKind::Call {
                func: Box::new(HirExpr::new(HirExprKind::Path(
                    super::expression::HirPath(syn::parse_str(&format!("::gourd::prelude::{rust_fn}")).unwrap()),
                ))),
                args,
            });
        }
    }

    // Handle os package method calls
    if receiver_str.contains("os") {
        let args: Vec<HirExpr> = method.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
        let rust_fn: &'static str = match method_name.as_str() {
            "Open" => "os_open",
            "ReadFile" => "os_read_file",
            "WriteFile" => "os_write_file",
            "Mkdir" => "os_mkdir",
            "MkdirAll" => "os_mkdir_all",
            "Remove" => "os_remove",
            "Chdir" => "os_chdir",
            "Getenv" => "os_getenv",
            "Setenv" => "os_setenv",
            _ => "SKIP"
        };
        if !rust_fn.is_empty() {
            return HirExpr::new(HirExprKind::Call {
                func: Box::new(HirExpr::new(HirExprKind::Path(
                    super::expression::HirPath(syn::parse_str(&format!("::gourd::prelude::{rust_fn}")).unwrap()),
                ))),
                args,
            });
        }
    }

    // Handle io package method calls
    if receiver_str.contains("io") {
        let args: Vec<HirExpr> = method.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
        let rust_fn: &'static str = match method_name.as_str() {
            "Copy" => "io_copy",
            "ReadAll" => "io_read_all",
            _ => "SKIP"
        };
        if !rust_fn.is_empty() {
            return HirExpr::new(HirExprKind::Call {
                func: Box::new(HirExpr::new(HirExprKind::Path(
                    super::expression::HirPath(syn::parse_str(&format!("::gourd::prelude::{rust_fn}")).unwrap()),
                ))),
                args,
            });
        }
    }

    // Handle bytes package method calls
    if receiver_str.contains("bytes") {
        let args: Vec<HirExpr> = method.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
        let rust_fn: &'static str = match method_name.as_str() {
            "Contains" => "bytes_contains",
            "HasPrefix" => "bytes_has_prefix",
            "HasSuffix" => "bytes_has_suffix",
            "Index" => "bytes_index",
            "Split" => "bytes_split",
            "Join" => "bytes_join",
            "Replace" => "bytes_replace",
            _ => "SKIP"
        };
        if !rust_fn.is_empty() {
            return HirExpr::new(HirExprKind::Call {
                func: Box::new(HirExpr::new(HirExprKind::Path(
                    super::expression::HirPath(syn::parse_str(&format!("::gourd::prelude::{rust_fn}")).unwrap()),
                ))),
                args,
            });
        }
    }

    // Handle json package method calls
    if receiver_str.contains("json") {
        let args: Vec<HirExpr> = method.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
        let rust_fn: &'static str = match method_name.as_str() {
            "Marshal" => "json_marshal",
            "Unmarshal" => "json_unmarshal",
            _ => "SKIP"
        };
        if !rust_fn.is_empty() {
            return HirExpr::new(HirExprKind::Call {
                func: Box::new(HirExpr::new(HirExprKind::Path(
                    super::expression::HirPath(syn::parse_str(&format!("::gourd::prelude::{rust_fn}")).unwrap()),
                ))),
                args,
            });
        }
    }

    // Handle time package method calls
    if receiver_str.contains("time") {
        let args: Vec<HirExpr> = method.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
        let rust_fn: &'static str = match method_name.as_str() {
            "Now" => "time_now",
            "Since" => "time_since",
            "Until" => "time_until",
            "Sleep" => "time_sleep",
            _ => "SKIP"
        };
        if !rust_fn.is_empty() {
            return HirExpr::new(HirExprKind::Call {
                func: Box::new(HirExpr::new(HirExprKind::Path(
                    super::expression::HirPath(syn::parse_str(&format!("::gourd::prelude::{rust_fn}")).unwrap()),
                ))),
                args,
            });
        }
    }

    // Handle byte package method calls
    if receiver_str.contains("byte") {
        let args: Vec<HirExpr> = method.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
        let rust_fn: &'static str = match method_name.as_str() {
            "Of" => "byte_of",
            "RuneOf" => "rune_of",
            "StringToBytes" => "string_to_bytes",
            "BytesToString" => "bytes_to_string",
            _ => "SKIP"
        };
        if !rust_fn.is_empty() {
            return HirExpr::new(HirExprKind::Call {
                func: Box::new(HirExpr::new(HirExprKind::Path(
                    super::expression::HirPath(syn::parse_str(&format!("::gourd::prelude::{rust_fn}")).unwrap()),
                ))),
                args,
            });
        }
    }
    let receiver = Box::new(go_ast_expr_to_hir(&method.receiver));
    let args: Vec<HirExpr> = method.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
    HirExpr::new(HirExprKind::MethodCall { receiver, method: method.method.clone(), args })
}

/// Convert a Go field access to HIR.
fn hir_field_to_hir(field: &ExprField) -> HirExpr {
    let receiver = Box::new(go_ast_expr_to_hir(&field.base));
    let field_name = match &field.member {
        syn::Member::Named(name) => name.clone(),
        syn::Member::Unnamed(idx) => {
            // Convert Index to a string for unnamed fields.
            // In syn 2, Index has an `index` field of type u32.
            let n = idx.index as usize;
            Ident::new(&format!("_{}", n), proc_macro2::Span::call_site())
        }
    };
    HirExpr::new(HirExprKind::FieldAccess { receiver, field: field_name })
}

/// Convert a Go index expression to HIR.
fn hir_index_to_hir(index: &ExprIndex) -> HirExpr {
    let collection = Box::new(go_ast_expr_to_hir(&index.expr));
    let index_expr = go_ast_expr_to_hir(&index.index);

    // If the index is a range expression, this is a slice operation
    if let HirExprKind::Slice { start, end, .. } = &index_expr.kind {
        // Use the slice with our actual collection instead of the dummy one
        return HirExpr::new(HirExprKind::Slice {
            collection,
            start: start.clone(),
            end: end.clone(),
        });
    }

    // Otherwise, it's a simple index access
    let index_expr = Box::new(index_expr);
    HirExpr::new(HirExprKind::Index { collection, index: index_expr })
}

/// Convert a Go parenthesized expression to HIR.
fn hir_paren_to_hir(paren: &ExprParen) -> HirExpr {
    go_ast_expr_to_hir(&paren.expr)
}

/// Convert a Go array literal to HIR.
fn hir_array_to_hir(array: &ExprArray) -> HirExpr {
    let elems: Vec<HirExpr> = array.elems.iter().map(|e| go_ast_expr_to_hir(e)).collect();
    // Go slice literal []T{...} → Rust vec![...]
    // Empty slice []int{} → vec![]
    HirExpr::new(HirExprKind::SliceLiteral(elems))
}

/// Convert a Go cast expression to HIR.
fn hir_cast_to_hir(cast: &ExprCast) -> HirExpr {
    let value = Box::new(go_ast_expr_to_hir(&cast.expr));
    let target_type = hir_type_from_syn(&cast.ty);
    HirExpr::new(HirExprKind::Cast { value, target_type })
}

/// Convert a Go assignment expression to HIR.
fn hir_assign_to_hir(assign: &ExprAssign) -> HirExpr {
    let target = Box::new(go_ast_expr_to_hir(&assign.left));
    let value = Box::new(go_ast_expr_to_hir(&assign.right));
    HirExpr::new(HirExprKind::Binary {
        op: HirBinaryOp::Assign,
        lhs: target,
        rhs: value,
    })
}

/// Convert a Go if expression to HIR.
fn hir_if_to_hir(if_expr: &ExprIf) -> HirExpr {
    let cond = Box::new(go_ast_expr_to_hir(&if_expr.cond));
    let then_body = hir_block_from_syn(&if_expr.then_branch);
    let else_body = if_expr.else_branch.as_ref()
        .map(|(_, branch)| {
            // The else branch can be any expression: a block, an if, or something else.
            // If it's a block, convert it directly. Otherwise, convert as an expression.
            match branch.as_ref() {
                Expr::Block(block) => Some(hir_block_from_syn(&block.block)),
                Expr::If(inner_if) => Some(hir_if_to_hir(inner_if).kind.unwrap_block()),
                _ => {
                    // Other expressions as else — convert to an Expr statement block
                    let expr = go_ast_expr_to_hir(branch);
                    Some(HirBlock { stmts: vec![HirStatement::Expr(Box::new(expr))] })
                }
            }
        })
        .flatten();
    HirExpr::new(HirExprKind::Block(HirBlock {
        stmts: vec![HirStatement::If {
            cond,
            then_body,
            else_body,
        }],
    }))
}

/// Convert a Go range expression to HIR.
fn hir_range_to_hir(range: &ExprRange) -> HirExpr {
    let start = range.start.as_ref().map(|e| Box::new(go_ast_expr_to_hir(e)));
    let end = range.end.as_ref().map(|e| Box::new(go_ast_expr_to_hir(e)));
    // Range on its own is usually used in `for` loops — treat as a slice operation
    // on a dummy range collection (the range is handled by the for-loop conversion)
    HirExpr::new(HirExprKind::Slice {
        collection: Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Nil))),
        start,
        end,
    })
}

/// Convert a Go pattern to HIR expression.
fn hir_pat_to_hir(pat: &Pat) -> HirExpr {
    match pat {
        Pat::Ident(ident) => HirExpr::new(HirExprKind::Identifier(ident.ident.clone())),
        Pat::Wild(_) => HirExpr::new(HirExprKind::Literal(HirLiteral::Nil)),
        Pat::Lit(_lit) => HirExpr::new(HirExprKind::Literal(HirLiteral::Int(0))), // fallback
        Pat::Tuple(tuple) => {
            let elems: Vec<HirExpr> = tuple.elems.iter().map(|e| hir_pat_to_hir(e)).collect();
            HirExpr::new(HirExprKind::Tuple(elems))
        }
        Pat::TupleStruct(tuple_struct) => {
            // PatTupleStruct has `elems` field (not `fields`)
            let elems: Vec<HirExpr> = tuple_struct
                .elems
                .iter()
                .map(|e| hir_pat_to_hir(e))
                .collect();
            HirExpr::new(HirExprKind::Tuple(elems))
        }
        Pat::Struct(struct_pat) => {
            // PatStruct has `fields` field with `FieldPat` entries
            let fields: Vec<HirExpr> = struct_pat
                .fields
                .iter()
                .filter_map(|field| {
                    Some(hir_pat_to_hir(&field.pat))
                })
                .collect();
            HirExpr::new(HirExprKind::Tuple(fields))
        }
        Pat::Path(path) => HirExpr::new(HirExprKind::Path(HirPath(path.path.clone()))),
        Pat::Reference(reference) => {
            let inner = hir_pat_to_hir(&reference.pat);
            HirExpr::new(HirExprKind::Unary {
                op: HirUnaryOp::Deref,
                operand: Box::new(inner),
            })
        }
        Pat::Verbatim(_) => {
            HirExpr::new(HirExprKind::Unsupported("box pattern".to_string()))
        }
        Pat::Slice(slice_pat) => {
            let elems: Vec<HirExpr> = slice_pat.elems.iter().map(|e| hir_pat_to_hir(e)).collect();
            HirExpr::new(HirExprKind::Tuple(elems))
        }
        Pat::Rest(_) => HirExpr::new(HirExprKind::Identifier(Ident::new("..", proc_macro2::Span::call_site()))),
        Pat::Or(or_pat) => {
            // Or pattern: `pat1 | pat2` — take the first alternative
            if or_pat.cases.is_empty() {
                HirExpr::new(HirExprKind::Literal(HirLiteral::Nil))
            } else {
                hir_pat_to_hir(or_pat.cases.first().unwrap())
            }
        }
        Pat::Paren(paren) => hir_pat_to_hir(&paren.pat),
        Pat::Type(type_pat) => hir_pat_to_hir(&type_pat.pat),
        Pat::Macro(macro_pat) => {
            HirExpr::new(HirExprKind::Macro(macro_pat.mac.tokens.clone()))
        }
        Pat::Const(_const_pat) => {
            HirExpr::new(HirExprKind::Literal(HirLiteral::Int(0))) // fallback
        }
        _ => HirExpr::new(HirExprKind::Literal(HirLiteral::Nil)), // fallback for unknown patterns
    }
}

/// Convert a Go loop expression to HIR.
fn hir_loop_to_hir(loop_expr: &ExprLoop) -> HirExpr {
    let body = hir_block_from_syn(&loop_expr.body);
    HirExpr::new(HirExprKind::Block(HirBlock {
        stmts: vec![HirStatement::While {
            cond: Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Bool(true)))),
            body,
        }],
    }))
}

/// Convert a Go for-loop expression to HIR.
fn hir_for_loop_to_hir(for_loop: &ExprForLoop) -> HirExpr {
    let iterable = Box::new(go_ast_expr_to_hir(&for_loop.expr));
    // Extract index and value names from the pattern
    let (index_name, value_name) = match &*for_loop.pat {
        Pat::Ident(ident) => (None, Some(ident.ident.clone())),
        Pat::Tuple(tuple) => {
            // Handle tuple patterns like (_, word) or (i, v)
            let idx = tuple.elems.first().and_then(|e| {
                if let Pat::Ident(ident) = e {
                    if ident.ident == "_" { None } else { Some(ident.ident.clone()) }
                } else { None }
            });
            let val = tuple.elems.get(1).and_then(|e| {
                if let Pat::Ident(ident) = e { Some(ident.ident.clone()) } else { None }
            });
            (idx, val)
        }
        _ => (None, None),
    };
    let body = hir_block_from_syn(&for_loop.body);
    HirExpr::new(HirExprKind::Block(HirBlock {
        stmts: vec![HirStatement::ForRange {
            index_name,
            value_name: value_name.unwrap_or_else(|| Ident::new("_", proc_macro2::Span::call_site())),
            iterable,
            body,
        }],
    }))
}

/// Convert a Go while expression to HIR.
fn hir_while_to_hir(while_expr: &ExprWhile) -> HirExpr {
    let cond = Box::new(go_ast_expr_to_hir(&while_expr.cond));
    let body = hir_block_from_syn(&while_expr.body);
    HirExpr::new(HirExprKind::Block(HirBlock {
        stmts: vec![HirStatement::While { cond, body }],
    }))
}

/// Convert a Go let expression to HIR.
fn hir_let_to_hir(let_expr: &ExprLet) -> HirExpr {
    let expr = Box::new(go_ast_expr_to_hir(&let_expr.expr));
    let name = match &*let_expr.pat {
        Pat::Ident(ident) => ident.ident.clone(),
        _ => Ident::new("_", proc_macro2::Span::call_site()),
    };
    HirExpr::new(HirExprKind::Block(HirBlock {
        stmts: vec![HirStatement::Local {
            name,
            mutable: true,
            value: expr,
        }],
    }))
}

/// Convert a Go tuple expression to HIR.
fn hir_tuple_to_hir(tuple: &ExprTuple) -> HirExpr {
    let elems: Vec<HirExpr> = tuple.elems.iter().map(|e| go_ast_expr_to_hir(e)).collect();
    HirExpr::new(HirExprKind::Tuple(elems))
}

/// Convert a Go break expression to HIR.
fn hir_break_to_hir(break_expr: &ExprBreak) -> HirExpr {
    HirExpr::new(HirExprKind::Block(HirBlock {
        stmts: vec![HirStatement::Break(break_expr.label.as_ref().map(|l| l.ident.clone()))],
    }))
}

/// Convert a Go continue expression to HIR.
fn hir_continue_to_hir(_cont: &ExprContinue) -> HirExpr {
    HirExpr::new(HirExprKind::Block(HirBlock {
        stmts: vec![HirStatement::Continue],
    }))
}

/// Convert a Go return expression to HIR.
fn hir_return_to_hir(ret: &ExprReturn) -> HirExpr {
    // Don't wrap in Block — just produce the return expression directly.
    // The HIR statement handler for Return will add the return keyword.
    if let Some(e) = &ret.expr {
        go_ast_expr_to_hir(e)
    } else {
        HirExpr::new(HirExprKind::Literal(HirLiteral::Nil))
    }
}

/// Convert a Go group expression to HIR.
fn hir_group_to_hir(group: &syn::ExprGroup) -> HirExpr {
    go_ast_expr_to_hir(&group.expr)
}

/// Convert a Go reference expression to HIR.
fn hir_reference_to_hir(reference: &ExprReference) -> HirExpr {
    let operand = Box::new(go_ast_expr_to_hir(&reference.expr));
    HirExpr::new(HirExprKind::Unary {
        op: HirUnaryOp::AddressOf,
        operand,
    })
}

/// Convert a Go closure expression to HIR.
fn hir_closure_to_hir_expr(closure: &ExprClosure) -> HirExpr {
    let params: Vec<(Ident, Option<Box<HirType>>)> = closure.inputs.iter()
        .map(|pat: &Pat| {
            match pat {
                Pat::Ident(ident) => {
                    let name = ident.ident.clone();
                    (name, None)
                }
                Pat::Type(pat_type) => {
                    let name = match &*pat_type.pat {
                        Pat::Ident(ident) => ident.ident.clone(),
                        _ => Ident::new("_", proc_macro2::Span::call_site()),
                    };
                    let ty = hir_type_from_syn(&pat_type.ty);
                    (name, Some(ty))
                }
                _ => (Ident::new("_", proc_macro2::Span::call_site()), None),
            }
        }).collect();
    let body = match &*closure.body {
        Expr::Block(block) => hir_block_from_syn(&block.block),
        other => {
            // Expression body (not a block) — wrap in a block
            let hir_expr = go_ast_expr_to_hir(other);
            HirBlock { stmts: vec![HirStatement::Expr(Box::new(hir_expr))] }
        }
    };
    let returns: Vec<Box<HirType>> = match &closure.output {
        syn::ReturnType::Type(_, ty) => vec![hir_type_from_syn(&**ty)],
        _ => vec![],
    };
    HirExpr::new(HirExprKind::Closure { params, returns, body })
}

/// Convert a Go struct expression to HIR.
fn hir_struct_to_hir(struct_expr: &syn::ExprStruct) -> HirExpr {
    // Struct literals: `StructName{field1: val1, field2: val2}`
    let name = struct_expr.path.clone();
    let fields: Vec<(syn::Ident, Box<HirExpr>)> = struct_expr
        .fields
        .iter()
        .map(|field_val| {
            let field_name = match &field_val.member {
                syn::Member::Named(ident) => ident.clone(),
                syn::Member::Unnamed(idx) => {
                    // Positional field: convert to named by position
                    syn::Ident::new(&format!("field{}", idx.index), proc_macro2::Span::call_site())
                }
            };
            let hir_value = go_ast_expr_to_hir(&field_val.expr);
            (field_name, Box::new(hir_value))
        })
        .collect();
    HirExpr::new(HirExprKind::StructLit { name, fields })
}

/// Convert a Go macro expression to HIR.
fn hir_macro_to_hir(macro_expr: &syn::ExprMacro) -> HirExpr {
    // Pass through macro tokens (vec!, format!, etc.)
    HirExpr::new(HirExprKind::Macro(macro_expr.mac.tokens.clone()))
}

/// Convert a Go match expression to HIR.
fn hir_match_to_hir(match_expr: &syn::ExprMatch) -> HirExpr {
    use syn::punctuated::Punctuated;
    // Match expressions: `match selector { arm1, arm2, ... }`
    let selector = Box::new(go_ast_expr_to_hir(&match_expr.expr));

    // Build match arms — single pattern per Rust arm
    let arms: Vec<(Vec<Box<HirExpr>>, HirBlock)> = match_expr
        .arms
        .iter()
        .map(|arm| {
            // Each arm has one pattern
            let patterns = vec![Box::new(hir_pat_to_hir(&arm.pat))];
            let body = if let Expr::Block(block_expr) = &*arm.body {
                hir_block_from_syn(&block_expr.block)
            } else {
                // Non-block body — treat as an expression in a block
                HirBlock {
                    stmts: vec![HirStatement::Expr(Box::new(go_ast_expr_to_hir(&arm.body)))],
                }
            };
            (patterns, body)
        })
        .collect();

    // Find default arm (wildcard pattern)
    let default_body = match_expr
        .arms
        .iter()
        .find(|arm| {
            matches!(&arm.pat, syn::Pat::Wild(_))
        })
        .map(|arm| {
            if let Expr::Block(block_expr) = &*arm.body {
                hir_block_from_syn(&block_expr.block)
            } else {
                HirBlock {
                    stmts: vec![HirStatement::Expr(Box::new(go_ast_expr_to_hir(&arm.body)))],
                }
            }
        });

    HirExpr::new(HirExprKind::Match {
        selector,
        arms,
        default_body,
    })
}

/// Convert a Go type to HIR.
pub(crate) fn hir_type_from_syn(ty: &Type) -> Box<HirType> {
    Box::new(hir_type_from_syn_impl(ty))
}

fn hir_type_from_syn_impl(ty: &Type) -> HirType {
    match ty {
        Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                let name = segment.ident.to_string();
                go_type_to_hir(&name)
            } else {
                HirType::new(HirTypeKind::Unknown("empty path".to_string()))
            }
        }
        Type::Reference(type_ref) => {
            let elem = hir_type_from_syn(&type_ref.elem);
            let lifetime = type_ref.lifetime.as_ref().map(|lt| lt.ident.clone());
            HirType::new(HirTypeKind::Reference(elem, lifetime))
        }
        Type::Slice(type_slice) => {
            let elem = hir_type_from_syn(&type_slice.elem);
            HirType::new(HirTypeKind::Slice(elem))
        }
        Type::Ptr(type_ptr) => {
            let elem = hir_type_from_syn(&type_ptr.elem);
            HirType::new(HirTypeKind::Pointer(elem))
        }
        Type::Tuple(_type_tuple) => {
            // Tuple types map to Rust tuples
            // For simplicity, map to Unknown
            HirType::new(HirTypeKind::Unknown("tuple".to_string()))
        }
        _ => HirType::new(HirTypeKind::Unknown("unsupported type".to_string())),
    }
}

/// Convert a Go block (syn::Block) to a HIR block.
fn hir_block_from_syn(block: &syn::Block) -> HirBlock {
    HirBlock {
        stmts: block.stmts.iter().map(|s| hir_stmt_from_syn(s)).collect(),
    }
}

/// Convert a Go statement (syn::Stmt) to a HIR statement.
fn hir_stmt_from_syn(stmt: &syn::Stmt) -> HirStatement {
    match stmt {
        syn::Stmt::Local(local) => {
            let name = match &local.pat {
                Pat::Ident(ident) => ident.ident.clone(),
                _ => Ident::new("_", proc_macro2::Span::call_site()),
            };
            let value = local.init.as_ref()
                .map(|init| Box::new(go_ast_expr_to_hir(&init.expr)))
                .unwrap_or_else(|| Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Nil))));
            // In syn 2, let_token is always present. mutable is indicated by whether
            // there's an init (:=) or not (=).
            let mutable = local.init.is_some();
            HirStatement::Local { name, mutable, value }
        }
        syn::Stmt::Expr(expr, _) => {
            let hir_expr = go_ast_expr_to_hir(expr);
            HirStatement::Expr(Box::new(hir_expr))
        }
        syn::Stmt::Item(item) => {
            // Items inside blocks: handle functions, structs, impls, etc.
            // For now, convert to a nop since Go doesn't have nested items in basic blocks.
            // In Go, `func` declarations are always at package level, not inside blocks.
            match &*item {
                syn::Item::Fn(fn_item) => {
                    // Nested function — convert to a closure and call it
                    let closure_params: Vec<(syn::Ident, Option<Box<HirType>>)> = fn_item
                        .sig
                        .inputs
                        .iter()
                        .filter_map(|input| match input {
                            syn::FnArg::Typed(pat_type) => {
                                let name = match &*pat_type.pat {
                                    Pat::Ident(ident) => Some(ident.ident.clone()),
                                    _ => None,
                                };
                                                let mut ty = hir_type_from_syn(&pat_type.ty);
                                    // Convert Slice → SliceRef for closure params (slices are always borrowed in Go)
                                    if matches!(ty.kind, HirTypeKind::Slice(_)) {
                                        let inner = match ty.kind { HirTypeKind::Slice(e) => e, _ => return None };
                                        ty = Box::new(HirType::new(HirTypeKind::SliceRef(inner)));
                                    }
                                let ty = Some(ty);
                                name.map(|n| (n, ty))
                            }
                            syn::FnArg::Receiver(_) => None,
                        })
                        .collect();
                    let closure_body = hir_block_from_syn(&fn_item.block);
                    let closure = HirExpr::new(HirExprKind::Closure {
                        params: closure_params,
                        returns: vec![],
                        body: closure_body,
                    });
                    HirStatement::Expr(Box::new(closure))
                }
                syn::Item::Struct(struct_item) => {
                    // Struct definition — convert to a unit type for now
                    let name = struct_item.ident.clone();
                    HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Identifier(name))))
                }
                syn::Item::Impl(impl_item) => {
                    // Impl block inside a go! block — emit as raw Rust impl tokens
                    // This preserves the method signatures and bodies correctly.
                    // Receiver methods are handled by top-level dispatch in lib.rs,
                    // but when embedded inside a block we emit them as raw tokens.
                    let rust_impl: TokenStream = impl_item.to_token_stream().into();
                    HirStatement::RawStmt { tokens: rust_impl }
                }
                syn::Item::Use(_use_item) => {
                    // Use statement — skip for now
                    HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Nil))))
                }
                syn::Item::Trait(_trait_item) => {
                    // Trait definition — skip for now
                    HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Nil))))
                }
                syn::Item::Enum(_enum_item) => {
                    // Enum definition — skip for now
                    HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Nil))))
                }
                syn::Item::Mod(_) => {
                    // Module definition — skip for now
                    HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Nil))))
                }
                syn::Item::Const(const_item) => {
                    // Const definition — convert to a local binding
                    let name = const_item.ident.clone();
                    let value = go_ast_expr_to_hir(&*const_item.expr);
                    HirStatement::Local {
                        name,
                        mutable: false,
                        value: Box::new(value),
                    }
                }
                syn::Item::Static(static_item) => {
                    // Static definition — convert to a mutable local binding
                    let name = static_item.ident.clone();
                    let value = Box::new(go_ast_expr_to_hir(&*static_item.expr));
                    HirStatement::Local {
                        name,
                        mutable: true,
                        value,
                    }
                }
                _ => HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Nil)))),
            }
        }
        _ => {
            HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Nil))))
        }
    }
}

/// Convert a Go AST statement to a HIR statement.
///
/// This is the core conversion function for GoStmt variants.
/// Each variant maps to the corresponding HIR statement type.
pub fn go_stmt_to_hir(stmt: &super::ast::GoStmt) -> HirStatement {
    use super::ast::{GoStmt, GoBlock};

    match stmt {
        GoStmt::Local(local) => {
            // Go short declaration `x := value`
            let name = match &local.pat {
                Pat::Ident(ident) => ident.ident.clone(),
                _ => Ident::new("_", proc_macro2::Span::call_site()),
            };
            let value = local.init.as_ref()
                .map(|init| Box::new(go_ast_expr_to_hir(&init.expr)))
                .unwrap_or_else(|| Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Nil))));
            let mutable = true; // `:=` always creates mutable bindings
            HirStatement::Local { name, mutable, value }
        }
        GoStmt::Expr(expr) => {
            // Expression statement: `foo(x)` (side-effect only)
            let hir_expr = go_ast_expr_to_hir(expr);
            HirStatement::Expr(Box::new(hir_expr))
        }
        GoStmt::If(go_if) => {
            let cond = Box::new(go_ast_expr_to_hir(&go_if.cond));
            let then_body = go_block_to_hir(&go_if.then_block);
            let else_body = go_if.else_block.as_ref().map(|b| go_block_to_hir(b));
            HirStatement::If {
                cond,
                then_body,
                else_body,
            }
        }
        GoStmt::While(while_stmt) => {
            let cond = Box::new(go_ast_expr_to_hir(&while_stmt.cond));
            let body = go_block_to_hir(&while_stmt.body);
            HirStatement::While { cond, body }
        }
        GoStmt::Continue => {
            HirStatement::Continue
        }
        GoStmt::GoFor(go_for) => {
            if go_for.is_range {
                // Range-based: `for i, v := range items { ... }`
                let iterable = go_for.iterable
                    .as_ref()
                    .map(|p| Box::new(go_ast_expr_to_hir(&syn::Expr::Path(syn::ExprPath {
                        attrs: Vec::new(),
                        qself: None,
                        path: p.clone(),
                    }))))
                    .unwrap_or_else(|| Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Nil))));

                // Handle init (index + value names)
                match &go_for.init {
                    Some(go_for_init) => {
                        match go_for_init {
                            super::ast::GoForInit::Single(ident, _) => {
                                // `for i := range items` — only index, no value
                                let range_vars = vec![ident.clone()];
                                HirStatement::ForRange {
                                    index_name: Some(ident.clone()),
                                    value_name: Ident::new("_", proc_macro2::Span::call_site()),
                                    iterable,
                                    body: go_block_to_hir_with_range_vars(&go_for.body, &range_vars),
                                }
                            }
                            super::ast::GoForInit::Double(ident1, ident2, _) => {
                                // `for i, v := range items`
                                let range_vars = vec![ident1.clone(), ident2.clone()];
                                HirStatement::ForRange {
                                    index_name: Some(ident1.clone()),
                                    value_name: ident2.clone(),
                                    iterable,
                                    body: go_block_to_hir_with_range_vars(&go_for.body, &range_vars),
                                }
                            }
                        }
                    }
                    None => {
                        // No init — just iterate
                        let value_name = Ident::new("_", proc_macro2::Span::call_site());
                        HirStatement::ForRange {
                            index_name: None,
                            value_name,
                            iterable,
                            body: go_block_to_hir(&go_for.body),
                        }
                    }
                }
            } else {
                // C-style: `for i := 0; i < n; i++ { ... }`
                // Init: `i := 0` → Local statement `let mut i = 0`
                let init: Option<Box<HirStatement>> = go_for.init.as_ref().map(|go_for_init| {
                    match go_for_init {
                        super::ast::GoForInit::Single(ident, value) => {
                            // `i := 0` — short declaration with init value
                            let value = value.as_ref()
                                .map(|v| Box::new(go_ast_expr_to_hir(v)))
                                .unwrap_or_else(|| Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Int(0)))));
                            Box::new(HirStatement::Local {
                                name: ident.clone(),
                                mutable: true,
                                value,
                            })
                        }
                        super::ast::GoForInit::Double(ident1, _ident2, value) => {
                            // `i, v := 0, 0` — double init. Generate separate local statements.
                            let value = value.as_ref()
                                .map(|v| go_ast_expr_to_hir(v))
                                .unwrap_or_else(|| HirExpr::new(HirExprKind::Literal(HirLiteral::Int(0))));
                            // For double init, only use first identifier (main loop index)
                            // Second variable is used inside body
                            Box::new(HirStatement::Local {
                                name: ident1.clone(),
                                mutable: true,
                                value: Box::new(value),
                            })
                        }
                    }
                });

                let condition = go_for.cond
                    .as_ref()
                    .map(|e| Box::new(go_ast_expr_to_hir(e)))
                    .unwrap_or_else(|| Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Bool(true)))));

                // Post: `i++` → `i = i + 1` as Expr statement
                let post: Option<Box<HirStatement>> = go_for.post.as_ref().map(|e| {
                    let expr = go_ast_expr_to_hir(e);
                    Box::new(HirStatement::Expr(Box::new(expr)))
                });

                HirStatement::ForLoop {
                    init,
                    condition,
                    post,
                    body: go_block_to_hir(&go_for.body),
                }
            }
        }
        GoStmt::GoChannelSend(chan, val) => {
            let channel = Box::new(go_ast_expr_to_hir(chan));
            let value = Box::new(go_ast_expr_to_hir(val));
            HirStatement::ChannelSend { channel, value }
        }
        GoStmt::GoChannelRecv(recv) => {
            let channel = Box::new(go_ast_expr_to_hir(recv));
            // Simple recv: `<-ch` — no target
            HirStatement::ChannelRecv {
                channel,
                target: None,
            }
        }
        GoStmt::GoTypeAssert(value, target_type) => {
            let value = Box::new(go_ast_expr_to_hir(value));
            // Convert syn::Type to HirType
            use super::types::go_type_to_hir;
            let hir_type_str = quote::quote! { #target_type }.to_string();
            let hir_type = go_type_to_hir(&hir_type_str);
            HirStatement::TypeAssert {
                value,
                target_type: Box::new(hir_type),
                result_name: None,
            }
        }
        GoStmt::GoReturn(exprs) => {
            if exprs.is_empty() {
                HirStatement::Return(None)
            } else if exprs.len() == 1 {
                let value = Box::new(go_ast_expr_to_hir(&exprs[0]));
                HirStatement::Return(Some(value))
            } else {
                // Multi-return: `return a, b`
                let values: Vec<HirExpr> = exprs.iter().map(|e| {
                    go_ast_expr_to_hir(e)
                }).collect();
                // Wrap in a tuple
                let tuple = HirExpr::new(HirExprKind::Tuple(values));
                HirStatement::Return(Some(Box::new(tuple)))
            }
        }
        GoStmt::Switch(sw) => {
            // Convert switch to a match expression
            eprintln!("DEBUG: go_stmt_to_hir Switch, selector={:?}, cases={}", sw.selector.is_some(), sw.cases.len());
            let selector = Box::new(
                sw.selector.as_ref()
                    .map(|e| go_ast_expr_to_hir(e))
                    .unwrap_or_else(|| HirExpr::new(HirExprKind::Literal(HirLiteral::Bool(true))))
            );

            // Build match arms — store all patterns for multi-pattern match arms (case 1, 2, 3)
            let arms: Vec<(Vec<Box<HirExpr>>, HirBlock)> = sw.cases.iter().map(|sc| {
                let patterns: Vec<Box<HirExpr>> = sc.exprs.iter().map(|e| {
                    Box::new(go_ast_expr_to_hir(e))
                }).collect();
                let stmts = sc.stmts.iter().map(|s| go_stmt_to_hir(s)).collect();
                let body = HirBlock { stmts };
                (patterns, body)
            }).collect();

            // Default case
            let default_body = if !sw.default_stmts.is_empty() {
                let stmts = sw.default_stmts.iter().map(|s| go_stmt_to_hir(s)).collect();
                Some(HirBlock { stmts })
            } else {
                None
            };

            HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Match {
                selector,
                arms,
                default_body,
            })))
        }
        GoStmt::Select(select_stmt) => {
            // Select statement: `select { case ... default: ... }`
            // Use the dedicated Go→Rust select handler which produces
            // `gourd::GoSelect::<T>::new() ... .run()` calls.
            let hir_select = super::go_select_to_hir(select_stmt);
            // Emit as a raw statement using the Rust runtime select primitive
            HirStatement::RawStmt {
                tokens: super::hir_select_to_rust_from_hir(&hir_select),
            }
        }
        GoStmt::Defer(closure_body) => {
            // `defer func() { ... }` → Rust Drop guard at end of scope
            // Parse the closure body as a block of statements
            let block: syn::Block = syn::parse2(closure_body.clone())
                .unwrap_or_else(|_| syn::parse_quote!({}));
            let body_stmts: Vec<HirStatement> = block.stmts.iter()
                .map(|s| match s {
                    syn::Stmt::Expr(expr, _) => {
                        HirStatement::Expr(Box::new(go_ast_expr_to_hir(&expr)))
                    }
                    syn::Stmt::Local(local) => {
                        // Convert syn::Local to HirStatement::Local
                        let name = match &local.pat {
                            syn::Pat::Ident(pat_ident) => pat_ident.ident.clone(),
                            _ => syn::Ident::new("_", proc_macro2::Span::call_site()),
                        };
                        let value = Box::new(go_ast_expr_to_hir(&local.init.as_ref().unwrap().expr));
                        HirStatement::Local {
                            name,
                            mutable: true,
                            value,
                        }
                    }
                    syn::Stmt::Item(_) => {
                        // Items in defer body are rare in Go, but handle them gracefully
                        HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Nil))))
                    }
                    _ => {
                        // Unknown statement types in defer — skip gracefully
                        HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Nil))))
                    },
                })
                .collect();
            HirStatement::Defer {
                body: HirBlock { stmts: body_stmts },
            }
        }
        GoStmt::GoMake(raw_args) => {
            // `make(...)` statement with raw argument string
            // Parse the arguments and generate appropriate HIR
            let args: Vec<String> = raw_args.to_string().split(',').map(|s| s.trim().to_string()).collect();
            if args.len() >= 2 {
                // make(type, len) or make(type, len, cap)
                let ty_str = &args[0];
                let len_str = &args[1];
                let cap_str = if args.len() > 2 {
                    Some(&args[2])
                } else {
                    None
                };
                
                // Convert type string to HirType
                use super::types::go_type_to_hir;
                let hir_type = go_type_to_hir(ty_str);
                
                // Convert length to HirExpr
                let len_expr: syn::Expr = syn::parse_str(len_str).unwrap_or_else(|_| syn::parse_quote!(0));
                let len_hir = go_ast_expr_to_hir(&len_expr);
                
                // Create MakeKind based on whether we have cap
                let make_kind = if let Some(cap_str) = cap_str {
                    let cap_expr: syn::Expr = syn::parse_str(cap_str).unwrap_or_else(|_| syn::parse_quote!(0));
                    let cap_hir = go_ast_expr_to_hir(&cap_expr);
                    HirExprKind::Make(MakeKind::SliceWithCap(
                        Box::new(hir_type),
                        Box::new(len_hir),
                        Box::new(cap_hir),
                    ))
                } else {
                    HirExprKind::Make(MakeKind::Slice(
                        Box::new(hir_type),
                        Box::new(len_hir),
                    ))
                };
                
                HirStatement::Expr(Box::new(HirExpr::new(make_kind)))
            } else {
                // Insufficient args for make — create a nil expression as fallback
                HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Nil))))
            }
        }
        GoStmt::GoSlice(elements) => {
            // Go slice literal: `[]T{elem1, elem2, ...}`
            let hir_elements: Vec<HirExpr> = elements.iter().map(|e| {
                go_ast_expr_to_hir(e)
            }).collect();
            HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::SliceLiteral(hir_elements))))
        }
        GoStmt::GoMap(_key_type, _val_type, _slice_elem, entries) => {
            // Go map literal: `map[K]V{key1: val1, key2: val2, ...}`
            // Convert entries to HIR (key-value pairs)
            let entries: Vec<(Box<HirExpr>, Box<HirExpr>)> = entries.iter().map(|(k, v)| {
                (Box::new(go_ast_expr_to_hir(k)), Box::new(go_ast_expr_to_hir(v)))
            }).collect();
            HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Map(entries))))
        }
        GoStmt::RawStmt(raw_tokens) => {
            // Raw token stream — pass through tokens
            HirStatement::RawStmt {
                tokens: raw_tokens.clone(),
            }
        }
        GoStmt::SwitchReturn(tokens) => {
            // Pre-transpiled switch return — pass through tokens
            HirStatement::SwitchReturn {
                tokens: tokens.clone(),
            }
        }
        GoStmt::GoIfErr(check_expr, body_stmts) => {
            // `if err != nil { ... }` error handling
            // This is a common Go pattern: check if an error is not nil
            let check_expr: syn::Expr = syn::parse2(check_expr.clone()).unwrap_or_else(|_| syn::parse_quote!(false));
            let cond = Box::new(go_ast_expr_to_hir(&check_expr));
            let body = go_block_to_hir_with_stmts(body_stmts.as_slice());
            HirStatement::If {
                cond,
                then_body: body,
                else_body: None,
            }
        }
        GoStmt::GoImport(import_decl) => {
            // Import declarations: `import "strings"`, `import s "strings"`, `import . "fmt"`, `import _ "os"`
            HirStatement::Import {
                alias: import_decl.alias.as_ref().map(|a| a.to_string()),
                path: import_decl.path.clone(),
                dot: import_decl.dot,
                blank: import_decl.blank,
            }
        }
        GoStmt::GoLocal(name, value_stream) => {
            // Go short declaration: `x := value` or `x = value`
            // Always run preprocessing first to catch Go slice/map literals
            let preprocessed = preprocess_go_slice_literals(value_stream.clone());
            let is_modified = preprocessed.to_string() != value_stream.to_string();

            // If preprocessing modified the output, use it exclusively
            if is_modified {
                if let Ok(value_expr) = syn::parse2::<syn::Expr>(preprocessed) {
                    return HirStatement::Local {
                        name: name.clone(),
                        mutable: true,
                        value: Box::new(go_ast_expr_to_hir(&value_expr)),
                    };
                }
            }
            // Try original parsing (only if preprocessing didn't modify)
            if !is_modified {
                if let Ok(value_expr) = syn::parse2::<syn::Expr>(value_stream.clone()) {
                    let value = Box::new(go_ast_expr_to_hir(&value_expr));
                    return HirStatement::Local {
                        name: name.clone(),
                        mutable: true,
                        value,
                    };
                }
            }
            // Fallback: store the original value stream as raw tokens
            let value = Box::new(HirExpr::new(HirExprKind::Macro(value_stream.clone())));
            HirStatement::Local {
                name: name.clone(),
                mutable: true,
                value,
            }
        }
        GoStmt::GoShortDecl(name, value_stream) => {
            // Go `:=` short declaration (non-closure)
            // Always run preprocessing first to catch Go slice/map literals
            let preprocessed = preprocess_go_slice_literals(value_stream.clone());
            let is_modified = preprocessed.to_string() != value_stream.to_string();

            // If preprocessing modified the output, use it exclusively
            if is_modified {
                if let Ok(value_expr) = syn::parse2::<syn::Expr>(preprocessed) {
                    return HirStatement::Local {
                        name: name.clone(),
                        mutable: true,
                        value: Box::new(go_ast_expr_to_hir(&value_expr)),
                    };
                }
            }
            // Try original parsing (only if preprocessing didn't modify)
            if !is_modified {
                if let Ok(value_expr) = syn::parse2::<syn::Expr>(value_stream.clone()) {
                    let value = Box::new(go_ast_expr_to_hir(&value_expr));
                    return HirStatement::Local {
                        name: name.clone(),
                        mutable: true,
                        value,
                    };
                }
            }
            // Fallback: store the original value stream as raw tokens
            let value = Box::new(HirExpr::new(HirExprKind::Macro(value_stream.clone())));
            HirStatement::Local {
                name: name.clone(),
                mutable: true,
                value,
            }
        }
    }
}

/// Convert a Go block to a HIR block.
pub fn go_block_to_hir(block: &GoBlock) -> HirBlock {
    go_block_to_hir_with_stmts(block.stmts.as_slice())
}

/// Convert Go statements to a HIR block with range variable context.
/// When an identifier matches a range variable name, it's converted to RangeVar instead of Identifier.
pub fn go_block_to_hir_with_range_vars(block: &GoBlock, range_vars: &[syn::Ident]) -> HirBlock {
    go_block_to_hir_with_stmts_and_range_vars(block.stmts.as_slice(), range_vars)
}

/// Convert Go statements to a HIR block.
pub fn go_block_to_hir_with_stmts(stmts: &[GoStmt]) -> HirBlock {
    let body_stmts: Vec<HirStatement> = stmts.iter().map(|stm| go_stmt_to_hir(stm)).collect();
    HirBlock { stmts: body_stmts }
}

/// Convert Go statements to a HIR block with range variable context.
pub fn go_block_to_hir_with_stmts_and_range_vars(stmts: &[GoStmt], range_vars: &[syn::Ident]) -> HirBlock {
    let body_stmts: Vec<HirStatement> = stmts.iter().map(|stm| go_stmt_to_hir_with_range_vars(stm, range_vars)).collect();
    HirBlock { stmts: body_stmts }
}

/// Convert a Go statement to HIR with range variable context.
/// When an identifier in the statement matches a range variable name,
/// it is converted to RangeVar instead of Identifier.
fn go_stmt_to_hir_with_range_vars(stmt: &super::ast::GoStmt, range_vars: &[syn::Ident]) -> HirStatement {
    let stmt = go_stmt_to_hir(stmt);
    // Transform the statement to convert identifiers to RangeVar
    transform_stmt_range_vars(stmt, range_vars)
}

/// Transform a HIR statement to convert identifiers to RangeVar expressions.
fn transform_stmt_range_vars(stmt: HirStatement, range_vars: &[syn::Ident]) -> HirStatement {
    if range_vars.is_empty() {
        return stmt;
    }
    match stmt {
        HirStatement::Expr(expr) => {
            let transformed_expr = transform_expr_range_vars(*expr, range_vars);
            HirStatement::Expr(Box::new(transformed_expr))
        }
        HirStatement::Local { name, mutable, value } => {
            let transformed_value = transform_expr_range_vars(*value, range_vars);
            HirStatement::Local { name, mutable, value: Box::new(transformed_value) }
        }
        HirStatement::Assign { target, value } => {
            let transformed_target = transform_expr_range_vars(*target, range_vars);
            let transformed_value = transform_expr_range_vars(*value, range_vars);
            HirStatement::Assign { target: Box::new(transformed_target), value: Box::new(transformed_value) }
        }
        HirStatement::If { cond, then_body, else_body } => {
            let transformed_cond = transform_expr_range_vars(*cond, range_vars);
            let transformed_then = transform_block_range_vars(then_body, range_vars);
            let transformed_else = else_body.map(|b| transform_block_range_vars(b, range_vars));
            HirStatement::If { cond: Box::new(transformed_cond), then_body: transformed_then, else_body: transformed_else }
        }
        HirStatement::While { cond, body } => {
            let transformed_cond = transform_expr_range_vars(*cond, range_vars);
            let transformed_body = transform_block_range_vars(body, range_vars);
            HirStatement::While { cond: Box::new(transformed_cond), body: transformed_body }
        }
        HirStatement::ForRange { index_name, value_name, iterable, body } => {
            let transformed_iterable = transform_expr_range_vars(*iterable, range_vars);
            let transformed_body = transform_block_range_vars(body, range_vars);
            HirStatement::ForRange { index_name, value_name, iterable: Box::new(transformed_iterable), body: transformed_body }
        }
        HirStatement::ForLoop { init, condition, post, body } => {
            let transformed_init = init.map(|s| Box::new(transform_stmt_range_vars(*s, range_vars)));
            let transformed_condition = transform_expr_range_vars(*condition, range_vars);
            let transformed_post = post.map(|s| Box::new(transform_stmt_range_vars(*s, range_vars)));
            let transformed_body = transform_block_range_vars(body, range_vars);
            HirStatement::ForLoop {
                init: transformed_init,
                condition: Box::new(transformed_condition),
                post: transformed_post,
                body: transformed_body,
            }
        }
        HirStatement::Return(val) => {
            let transformed_val = val.map(|v| Box::new(transform_expr_range_vars(*v, range_vars)));
            HirStatement::Return(transformed_val)
        }
        other => other,
    }
}

/// Transform a HIR block to convert identifiers to RangeVar expressions.
fn transform_block_range_vars(block: HirBlock, range_vars: &[syn::Ident]) -> HirBlock {
    let transformed_stmts: Vec<HirStatement> = block
        .stmts
        .iter()
        .map(|s| transform_stmt_range_vars(s.clone(), range_vars))
        .collect();
    HirBlock { stmts: transformed_stmts }
}

/// Transform a HIR expression to convert identifiers to RangeVar expressions.
fn transform_expr_range_vars(expr: HirExpr, range_vars: &[syn::Ident]) -> HirExpr {
    if range_vars.is_empty() {
        return expr;
    }
    // Check if this is an identifier that needs range var conversion
    // before destructuring the expression
    if let HirExprKind::Identifier(ref id) = expr.kind {
        if range_vars.iter().any(|rv| rv == id) {
            let id = match expr.kind {
                HirExprKind::Identifier(id) => id,
                _ => unreachable!(),
            };
            return HirExpr::new(HirExprKind::RangeVar(id));
        }
    }
    // Not a range variable identifier, transform recursively
    match expr.kind {
        HirExprKind::Identifier(_) => expr,
        HirExprKind::Binary { op, lhs, rhs } => HirExpr::new(HirExprKind::Binary {
            op,
            lhs: Box::new(transform_expr_range_vars(*lhs, range_vars)),
            rhs: Box::new(transform_expr_range_vars(*rhs, range_vars)),
        }),
        HirExprKind::Unary { op, operand } => HirExpr::new(HirExprKind::Unary {
            op,
            operand: Box::new(transform_expr_range_vars(*operand, range_vars)),
        }),
        HirExprKind::Call { func, args } => HirExpr::new(HirExprKind::Call {
            func: Box::new(transform_expr_range_vars(*func, range_vars)),
            args: args
                .into_iter()
                .map(|a| transform_expr_range_vars(a, range_vars))
                .collect(),
        }),
        HirExprKind::MethodCall { receiver, method, args } => {
            HirExpr::new(HirExprKind::MethodCall {
                receiver: Box::new(transform_expr_range_vars(*receiver, range_vars)),
                method,
                args: args
                    .into_iter()
                    .map(|a| transform_expr_range_vars(a, range_vars))
                    .collect(),
            })
        }
        HirExprKind::FieldAccess { receiver, field } => HirExpr::new(HirExprKind::FieldAccess {
            receiver: Box::new(transform_expr_range_vars(*receiver, range_vars)),
            field,
        }),
        HirExprKind::Index { collection, index } => HirExpr::new(HirExprKind::Index {
            collection: Box::new(transform_expr_range_vars(*collection, range_vars)),
            index: Box::new(transform_expr_range_vars(*index, range_vars)),
        }),
        HirExprKind::Slice { collection, start, end } => HirExpr::new(HirExprKind::Slice {
            collection: Box::new(transform_expr_range_vars(*collection, range_vars)),
            start: start.map(|s| Box::new(transform_expr_range_vars(*s, range_vars))),
            end: end.map(|e| Box::new(transform_expr_range_vars(*e, range_vars))),
        }),
        HirExprKind::Cast { value, target_type } => HirExpr::new(HirExprKind::Cast {
            value: Box::new(transform_expr_range_vars(*value, range_vars)),
            target_type,
        }),
        HirExprKind::Tuple(elems) => {
            HirExpr::new(HirExprKind::Tuple(
                elems.into_iter().map(|e| transform_expr_range_vars(e, range_vars)).collect(),
            ))
        }
        HirExprKind::Block(block) => {
            HirExpr::new(HirExprKind::Block(transform_block_range_vars(block, range_vars)))
        }
        HirExprKind::Closure { params, returns: _, body } => HirExpr::new(HirExprKind::Closure {
            params,
            returns: vec![],
            body: transform_block_range_vars(body, range_vars),
        }),
        other => HirExpr::new(other),
    }
}

// ─── Select and Switch conversion ─────────────────────────────────────────────

/// Convert a Go select statement to HIR.
pub fn go_select_to_hir(select: &GoSelect) -> HirSelect {
    let cases: Vec<HirSelectCase> = select.cases.iter()
        .map(|case| match case {
            GoSelectCase::Send { ch, value } => {
                let ch_expr: Expr = syn::parse2(ch.as_ref().clone()).unwrap_or_else(|_| syn::parse_str("()").unwrap());
                let val_expr: Expr = syn::parse2(value.as_ref().clone()).unwrap_or_else(|_| syn::parse_str("()").unwrap());
                HirSelectCase::Send {
                    ch: Box::new(go_ast_expr_to_hir(&ch_expr)),
                    value: Box::new(go_ast_expr_to_hir(&val_expr)),
                }
            }
            GoSelectCase::Recv { ch, target: _ } => {
                let ch_expr: Expr = syn::parse2(ch.as_ref().clone()).unwrap_or_else(|_| syn::parse_str("()").unwrap());
                HirSelectCase::Recv {
                    ch: Box::new(go_ast_expr_to_hir(&ch_expr)),
                }
            }
            GoSelectCase::Default(_) => {
                HirSelectCase::Default
            }
        })
        .collect();

    let default_body = select.cases.iter()
        .find_map(|case| {
            if let GoSelectCase::Default(block) = case {
                Some(go_block_to_hir(block))
            } else { None }
        });

    HirSelect { cases, default_body }
}

/// Convert a Go switch statement to HIR.
pub fn go_switch_to_hir(switch: &Switch) -> HirSwitch {
    let cases: Vec<HirSwitchCase> = switch.cases.iter()
        .map(|case| {
            let patterns: Vec<HirExpr> = case.exprs.iter()
                .map(|e| go_ast_expr_to_hir(e))
                .collect();
            let body = go_block_to_hir_with_stmts(case.stmts.as_slice());
            HirSwitchCase { patterns, body }
        })
        .collect();

    let default_body = if switch.default_stmts.is_empty() {
        None
    } else {
        Some(go_block_to_hir_with_stmts(&switch.default_stmts.as_slice()))
    };

    let selector = switch.selector.as_ref()
        .map(|s| Box::new(go_ast_expr_to_hir(s)));

    HirSwitch { selector, cases, default_body }
}

#[cfg(test)]
mod tests {
    use crate::transpiler::hir::ast::{GoBlock, GoSelect, GoSelectCase, Switch, SwitchCase, GoStmt, GoFor, GoIf, GoImport, GoWhile, GoForInit};
    use super::{HirStatement, HirTypeKind};
    use super::{ go_stmt_to_hir, go_block_to_hir, go_ast_expr_to_hir };
    use super::super::expression::{HirExpr, HirExprKind, HirLiteral, MakeKind};
    use super::super::types::HirType;
    use super::super::statement::HirBlock;
    use syn::{Ident, Expr, Pat, PatIdent};
    use proc_macro2::TokenStream;
    use quote::quote;

    #[test]
    fn test_go_stmt_return_single() {
        // `return value`
        let expr = syn::parse_quote!(42);
        let hir = go_stmt_to_hir(&GoStmt::GoReturn(vec![expr]));
        match hir {
            HirStatement::Return(Some(value)) => {
                match value.kind {
                    HirExprKind::Literal(HirLiteral::Int(n)) => {
                        assert_eq!(n, 42);
                    }
                    _ => panic!("Expected int literal"),
                }
            }
            _ => panic!("Expected Return statement"),
        }
    }

    #[test]
    fn test_go_stmt_expr() {
        // `foo(x)`
        let expr: syn::Expr = syn::parse_quote!(foo(x));
        let hir = go_stmt_to_hir(&GoStmt::Expr(expr));
        match hir {
            HirStatement::Expr(_) => {},
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_go_stmt_continue() {
        let hir = go_stmt_to_hir(&GoStmt::Continue);
        match hir {
            HirStatement::Continue => {},
            _ => panic!("Expected Continue statement"),
        }
    }

    #[test]
    fn test_go_block_to_hir() {
        // Empty block
        let block = GoBlock { stmts: Vec::new() };
        let hir_block = go_block_to_hir(&block);
        assert!(hir_block.is_empty());
    }

    #[test]
    fn test_go_stmt_return_empty() {
        // `return`
        let hir = go_stmt_to_hir(&GoStmt::GoReturn(Vec::new()));
        match hir {
            HirStatement::Return(None) => {},
            _ => panic!("Expected Return(None)"),
        }
    }

    #[test]
    fn test_go_stmt_make_slice() {
        // `make([]int, 10)`
        let args_str = "[]int, 10".to_string();
        let hir = go_stmt_to_hir(&GoStmt::GoMake(args_str));
        match hir {
            HirStatement::Expr(expr) => {
                match expr.kind {
                    HirExprKind::Make(MakeKind::Slice(_, _)) => {},
                    _ => panic!("Expected Make expression"),
                }
            }
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_go_stmt_slice_literal() {
        // `[]int{1, 2, 3}`
        let elements = vec![
            syn::parse_quote!(1),
            syn::parse_quote!(2),
            syn::parse_quote!(3),
        ];
        let hir = go_stmt_to_hir(&GoStmt::GoSlice(elements));
        match hir {
            HirStatement::Expr(expr) => {
                match expr.kind {
                    HirExprKind::SliceLiteral(elements) => {
                        assert_eq!(elements.len(), 3);
                    }
                    _ => panic!("Expected SliceLiteral expression"),
                }
            }
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_go_stmt_channel_send() {
        // `ch <- value`
        let ch: syn::Expr = syn::parse_quote!(ch);
        let value: syn::Expr = syn::parse_quote!(42);
        let hir = go_stmt_to_hir(&GoStmt::GoChannelSend(ch, value));
        match hir {
            HirStatement::ChannelSend { channel, value } => {
                // Check that we have channel and value expressions
                assert!(matches!(channel.kind, HirExprKind::Identifier(_)));
                assert!(matches!(value.kind, HirExprKind::Literal(_)));
            }
            _ => panic!("Expected ChannelSend statement"),
        }
    }

    #[test]
    fn test_go_stmt_channel_recv() {
        // `<-ch`
        let ch: syn::Expr = syn::parse_quote!(ch);
        let hir = go_stmt_to_hir(&GoStmt::GoChannelRecv(ch));
        match hir {
            HirStatement::ChannelRecv { channel, target } => {
                // Check that we have channel expression
                assert!(matches!(channel.kind, HirExprKind::Identifier(_)));
                assert!(target.is_none());
            }
            _ => panic!("Expected ChannelRecv statement"),
        }
    }

    #[test]
    fn test_go_stmt_if_err() {
        // `if err != nil { ... }`
        let check_stream: TokenStream = quote! { err != nil };
        let body_stmts: Vec<GoStmt> = vec![
            GoStmt::Expr(syn::parse_quote!(panic(err))),
        ];
        let hir = go_stmt_to_hir(&GoStmt::GoIfErr(check_stream, body_stmts));
        match hir {
            HirStatement::If { cond, then_body, else_body } => {
                assert!(else_body.is_none());
                assert!(!then_body.is_empty());
            }
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_go_stmt_map_literal() {
        // `map[string]int{"a": 1, "b": 2}`
        let entries: Vec<(syn::Expr, syn::Expr)> = vec![
            (syn::parse_quote!("a"), syn::parse_quote!(1)),
            (syn::parse_quote!("b"), syn::parse_quote!(2)),
        ];
        let hir = go_stmt_to_hir(&GoStmt::GoMap(
            "string".to_string(),
            Some(Box::new(syn::parse_str::<syn::Type>("int").unwrap())),
            None,
            entries,
        ));
        match hir {
            HirStatement::Expr(expr) => {
                match expr.kind {
                    HirExprKind::Map(entries) => {
                        assert_eq!(entries.len(), 2);
                    }
                    _ => panic!("Expected Map expression"),
                }
            }
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_go_stmt_local() {
        // `x := 42`
        let name = Ident::new("x", proc_macro2::Span::call_site());
        let value_stream: TokenStream = quote! { 42 };
        let hir = go_stmt_to_hir(&GoStmt::GoLocal(name.clone(), value_stream));
        match hir {
            HirStatement::Local { name: result_name, mutable, value } => {
                assert_eq!(result_name.to_string(), "x");
                assert!(mutable);
                match value.kind {
                    HirExprKind::Literal(HirLiteral::Int(n)) => {
                        assert_eq!(n, 42);
                    }
                    _ => panic!("Expected int literal"),
                }
            }
            _ => panic!("Expected Local statement"),
        }
    }

    #[test]
    fn test_go_stmt_short_decl() {
        // `x := value`
        let name = Ident::new("x", proc_macro2::Span::call_site());
        let value_stream: TokenStream = quote! { 42 };
        let hir = go_stmt_to_hir(&GoStmt::GoShortDecl(name.clone(), value_stream));
        match hir {
            HirStatement::Local { name: result_name, mutable, value } => {
                assert_eq!(result_name.to_string(), "x");
                assert!(mutable);
                match value.kind {
                    HirExprKind::Literal(HirLiteral::Int(n)) => {
                        assert_eq!(n, 42);
                    }
                    _ => panic!("Expected int literal"),
                }
            }
            _ => panic!("Expected Local statement"),
        }
    }

    #[test]
    fn test_go_stmt_break() {
        // `break` - handled via unsupported since GoStmt doesn't have Break variant
        // This tests that the unsupported fallback works correctly
        let hir = go_stmt_to_hir(&GoStmt::Expr(syn::parse_quote!(break)));
        match hir {
            HirStatement::Expr(_) => {},
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_go_block_if_with_else() {
        // `if x > 0 { "yes" } else { "no" }`
        let cond: syn::Expr = syn::parse_quote!(x > 0);
        let then_body: GoBlock = GoBlock {
            stmts: vec![GoStmt::GoReturn(vec![syn::parse_quote!("yes")])],
        };
        let else_body: GoBlock = GoBlock {
            stmts: vec![GoStmt::GoReturn(vec![syn::parse_quote!("no")])],
        };
        let hir = go_stmt_to_hir(&GoStmt::If(GoIf {
            cond,
            then_block: then_body,
            else_block: Some(else_body),
        }));
        match hir {
            HirStatement::If { cond, then_body, else_body } => {
                assert!(!then_body.is_empty());
                assert!(else_body.is_some());
                assert!(!else_body.unwrap().is_empty());
            }
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_go_block_for_range() {
        // `for i, v := range items { ... }`
        let init = GoForInit::Double(
            Ident::new("i", proc_macro2::Span::call_site()),
            Ident::new("v", proc_macro2::Span::call_site()),
            None,
        );
        let iterable: syn::Path = syn::parse_quote!(items);
        let body: GoBlock = GoBlock {
            stmts: vec![GoStmt::Expr(syn::parse_quote!(println!(v)))],
        };
        let hir = go_stmt_to_hir(&GoStmt::GoFor(GoFor {
            init: Some(init),
            is_range: true,
            iterable: Some(iterable),
            cond: None,
            post: None,
            body,
        }));
        match hir {
            HirStatement::ForRange {
                index_name: idx,
                value_name: val,
                iterable: iter,
                body: b,
            } => {
                assert!(idx.is_some());
                assert_eq!(val.to_string(), "v");
                assert!(!b.is_empty());
            }
            _ => panic!("Expected ForRange statement"),
        }
    }

    #[test]
    fn test_go_stmt_type_assertion() {
        // `x.(string)`
        let expr: syn::Expr = syn::parse_quote!(x);
        let ty: syn::Type = syn::parse_str("string").unwrap();
        let hir = go_stmt_to_hir(&GoStmt::GoTypeAssert(expr, ty));
        match hir {
            HirStatement::TypeAssert { value, target_type, result_name } => {
                assert!(matches!(value.kind, HirExprKind::Identifier(_)));
                assert!(result_name.is_none());
            }
            _ => panic!("Expected TypeAssert statement"),
        }
    }

    #[test]
    fn test_go_stmt_select() {
        // `select { case ch <- v: ... default: ... }`
        let cases: Vec<GoSelectCase> = vec![
            GoSelectCase::Send {
                ch: Box::new(quote! { ch }),
                value: Box::new(quote! { v }),
            },
        ];
        let hir = go_stmt_to_hir(&GoStmt::Select(GoSelect {
            cases,
        }));
        match hir {
            HirStatement::RawStmt { tokens } => {
                // Verify that select produces non-empty raw tokens
                // (the dedicated GoSelect handler emits gourd::GoSelect calls)
                let token_str = tokens.to_string();
                assert!(!token_str.is_empty(), "Select should produce non-empty tokens");
            }
            _ => panic!("Expected RawStmt for select (uses dedicated handler)"),
        }
    }

    #[test]
    fn test_go_stmt_assign() {
        // `x = y + z` - handled via Expr since GoStmt doesn't have Assign variant
        // This tests that the unsupported fallback works correctly
        let expr: syn::Expr = syn::parse_quote!(x = y + z);
        let hir = go_stmt_to_hir(&GoStmt::Expr(expr));
        match hir {
            HirStatement::Expr(_) => {},
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_go_stmt_while() {
        // `while i < 10 { i = i + 1 }`
        let cond: syn::Expr = syn::parse_quote!(i < 10);
        let body: GoBlock = GoBlock {
            stmts: vec![GoStmt::Expr(syn::parse_quote!(i = i + 1))],
        };
        let hir = go_stmt_to_hir(&GoStmt::While(GoWhile {
            cond,
            body,
        }));
        match hir {
            HirStatement::While { cond, body } => {
                assert!(!body.is_empty());
            }
            _ => panic!("Expected While statement"),
        }
    }

    #[test]
    fn test_go_stmt_raw() {
        // Raw token stream - pass through tokens
        let raw: TokenStream = quote! { x = y + z; };
        let hir = go_stmt_to_hir(&GoStmt::RawStmt(raw.clone()));
        match hir {
            HirStatement::RawStmt { tokens } => {
                assert_eq!(tokens.to_string(), raw.to_string());
            }
            _ => panic!("Expected RawStmt statement"),
        }
    }

    #[test]
    fn test_go_stmt_switch() {
        // `switch x { case 1: "one" case 2: "two" default: "other" }`
        let cases: Vec<SwitchCase> = vec![
            SwitchCase {
                exprs: vec![syn::parse_quote!(1)],
                stmts: vec![GoStmt::GoReturn(vec![syn::parse_quote!("one")])],
            },
            SwitchCase {
                exprs: vec![syn::parse_quote!(2)],
                stmts: vec![GoStmt::GoReturn(vec![syn::parse_quote!("two")])],
            },
            SwitchCase {
                exprs: vec![],
                stmts: vec![GoStmt::GoReturn(vec![syn::parse_quote!("other")])],
            },
        ];
        let default_stmts: Vec<GoStmt> = vec![];
        let switch = Switch {
            selector: Some(syn::parse_quote!(x)),
            cases,
            default_stmts,
        };
        let hir = go_stmt_to_hir(&GoStmt::Switch(switch));
        match hir {
            HirStatement::Expr(expr) => {
                match expr.kind {
                    HirExprKind::Match { selector, arms, default_body } => {
                        // selector is Box<HirExpr>, not Option — unwrap to check it
                        assert!(matches!(selector.kind, HirExprKind::Identifier(_)));
                        assert_eq!(arms.len(), 3);
                        assert!(default_body.is_none());
                    }
                    _ => panic!("Expected Match expression for Switch"),
                }
            }
            _ => panic!("Expected Expr statement"),
        }
    }

    #[test]
    fn test_go_stmt_switch_return() {
        // `return switch ...` pre-transpiled match
        let raw: TokenStream = quote! { match x { 1 => "one" _ => "other" } };
        let hir = go_stmt_to_hir(&GoStmt::SwitchReturn(raw.clone()));
        match hir {
            HirStatement::SwitchReturn { tokens } => {
                assert_eq!(tokens.to_string(), raw.to_string());
            }
            _ => panic!("Expected SwitchReturn statement"),
        }
    }

    #[test]
    fn test_go_stmt_defer() {
        // `defer func() { ... }` -> Rust Drop guard
        let raw: TokenStream = quote! { { x = 42; } };
        let hir = go_stmt_to_hir(&GoStmt::Defer(raw));
        match hir {
            HirStatement::Defer { body } => {
                assert_eq!(body.stmts.len(), 1);
            }
            _ => panic!("Expected Defer statement"),
        }
    }

    #[test]
    fn test_go_stmt_import_default() {
        // `import "strings"` - default package import
        let import_decl = GoImport {
            alias: None,
            dot: false,
            blank: false,
            path: "strings".to_string(),
        };
        let hir = go_stmt_to_hir(&GoStmt::GoImport(import_decl));
        match hir {
            HirStatement::Import { alias, path, dot, blank } => {
                assert!(alias.is_none());
                assert_eq!(path, "strings");
                assert!(!dot);
                assert!(!blank);
            }
            _ => panic!("Expected Import statement"),
        }
    }

    #[test]
    fn test_go_stmt_import_with_alias() {
        // `import s "strings"` - aliased import
        let import_decl = GoImport {
            alias: Some(Ident::new("s", proc_macro2::Span::call_site())),
            dot: false,
            blank: false,
            path: "strings".to_string(),
        };
        let hir = go_stmt_to_hir(&GoStmt::GoImport(import_decl));
        match hir {
            HirStatement::Import { alias, path, .. } => {
                assert_eq!(alias, Some("s".to_string()));
                assert_eq!(path, "strings");
            }
            _ => panic!("Expected Import statement"),
        }
    }

    #[test]
    fn test_go_stmt_import_dot() {
        // `import . "fmt"` - dot import
        let import_decl = GoImport {
            alias: None,
            dot: true,
            blank: false,
            path: "fmt".to_string(),
        };
        let hir = go_stmt_to_hir(&GoStmt::GoImport(import_decl));
        match hir {
            HirStatement::Import { dot, .. } => {
                assert!(dot);
            }
            _ => panic!("Expected Import statement"),
        }
    }

    #[test]
    fn test_go_stmt_import_blank() {
        // `import _ "os"` - blank import
        let import_decl = GoImport {
            alias: None,
            dot: false,
            blank: true,
            path: "os".to_string(),
        };
        let hir = go_stmt_to_hir(&GoStmt::GoImport(import_decl));
        match hir {
            HirStatement::Import { blank, .. } => {
                assert!(blank);
            }
            _ => panic!("Expected Import statement"),
        }
    }

    #[test]
    fn test_go_stmt_local_syn() {
        // `x := 42` via syn::Local (constructed manually)
        let pat_ident: PatIdent = PatIdent {
            attrs: Vec::new(),
            by_ref: None,
            mutability: None,
            ident: Ident::new("x", proc_macro2::Span::call_site()),
            subpat: None,
        };
        let expr: Expr = syn::parse_quote!(42);
        let local_init = syn::LocalInit {
            eq_token: syn::token::Eq::default(),
            expr: Box::new(expr),
            diverge: None,
        };
        let local = syn::Local {
            attrs: Vec::new(),
            let_token: syn::token::Let::default(),
            pat: Pat::Ident(pat_ident),
            init: Some(local_init),
            semi_token: syn::token::Semi::default(),
        };
        let hir = go_stmt_to_hir(&GoStmt::Local(local));
        match hir {
            HirStatement::Local { name: result_name, mutable, value } => {
                assert_eq!(result_name.to_string(), "x");
                assert!(mutable);
                match value.kind {
                    HirExprKind::Literal(HirLiteral::Int(n)) => {
                        assert_eq!(n, 42);
                    }
                    _ => panic!("Expected int literal"),
                }
            }
            _ => panic!("Expected Local statement"),
        }
    }


}

#[cfg(test)]
mod std_parsing_test {
    use super::*;
    use proc_macro2::TokenStream;

    #[test]
    fn test_std_copy_path_segments() {
        let code = "std::copy(dst, src)";
        match syn::parse_str::<Expr>(code) {
            Ok(Expr::Call(call)) => {
                if let Expr::Path(path) = &*call.func {
                    assert_eq!(path.path.segments.len(), 2);
                    assert_eq!(path.path.segments[0].ident.to_string(), "std");
                    assert_eq!(path.path.segments[1].ident.to_string(), "copy");
                } else {
                    panic!("Expected Call expression");
                }
            }
            _ => panic!("Expected Call expression"),
        }
    }

    #[test]
    fn test_parse_quote_std_call() {
        let method_name: syn::Ident = syn::parse_str("copy").unwrap();
        let args_ts: TokenStream = "(dst, src)".parse().unwrap();
        let full_expr: Expr = syn::parse2(quote! { std :: #method_name #args_ts }).unwrap();
        match &full_expr {
            Expr::Call(call) => {
                if let Expr::Path(path) = &*call.func {
                    assert_eq!(path.path.segments.len(), 2);
                } else {
                    panic!("Expected Path as func");
                }
            }
            _ => panic!("Expected Call expression"),
        }
    }
}
