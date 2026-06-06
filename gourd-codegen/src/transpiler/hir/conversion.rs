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
use super::expression::*;
use super::types::*;
use crate::transpiler::types::map_go_types;
use super::statement::*;
use crate::transpiler::ast::{GoBlock, GoStmt, GoForInit, Switch, GoImport};

/// Convert a Go `syn::Expr` to a HIR expression.
///
/// This is the core conversion function. It walks the `syn::Expr` tree
/// and converts each node to the corresponding HIR node.
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
        _ => HirExpr::new(HirExprKind::Unsupported("unsupported Go expression".to_string())),
    }
}

/// Convert a Go literal to HIR.
fn hir_lit_to_hir(lit: &ExprLit) -> HirExpr {
    let lit = &lit.lit;
    match lit {
        syn::Lit::Str(s) => HirExpr::new(HirExprKind::Literal(HirLiteral::StringTy(s.value()))),
        syn::Lit::Int(n) => HirExpr::new(HirExprKind::Literal(HirLiteral::Int(n.base10_parse().unwrap_or(0)))),
        syn::Lit::Float(f) => HirExpr::new(HirExprKind::Literal(HirLiteral::Float(f.base10_parse().unwrap_or(0.0)))),
        syn::Lit::Bool(b) => HirExpr::new(HirExprKind::Literal(HirLiteral::Bool(b.value))),
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
    let op = hir_binary_op_to_hir(&binary.op);
    let lhs = Box::new(go_ast_expr_to_hir(&binary.left));
    let rhs = Box::new(go_ast_expr_to_hir(&binary.right));
    HirExpr::new(HirExprKind::Binary { op, lhs, rhs })
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
        BinOp::Shl(_) => HirBinaryOp::Lsh,
        BinOp::Shr(_) => HirBinaryOp::Rsh,
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
    // Check for Go type conversion calls: int(), string(), bool(), etc.
    if let Expr::Path(path) = &*call.func {
        if let Some(name) = path.path.get_ident() {
            let name_str = name.to_string();
            let is_type_convert = matches!(
                name_str.as_str(),
                "int" | "int8" | "int16" | "int32" | "int64"
                | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr"
                | "float32" | "float64" | "bool" | "byte" | "rune" | "string"
            );
            if is_type_convert {
                if let Some(arg) = call.args.first() {
                    let arg_expr = go_ast_expr_to_hir(arg);
                    return HirExpr::new(HirExprKind::TypeConvert { func: name.clone(), arg: Box::new(arg_expr) });
                }
            }
        }
    }
    let func = Box::new(go_ast_expr_to_hir(&call.func));
    let args: Vec<HirExpr> = call.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
    HirExpr::new(HirExprKind::Call { func, args })
}

/// Convert a Go method call to HIR.
fn hir_method_call_to_hir(method: &ExprMethodCall) -> HirExpr {
    let receiver = Box::new(go_ast_expr_to_hir(&method.receiver));
    let method_name = method.method.clone();
    let args: Vec<HirExpr> = method.args.iter().map(|a| go_ast_expr_to_hir(a)).collect();
    HirExpr::new(HirExprKind::MethodCall { receiver, method: method_name, args })
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
    let index_expr = Box::new(go_ast_expr_to_hir(&index.index));
    HirExpr::new(HirExprKind::Index { collection, index: index_expr })
}

/// Convert a Go parenthesized expression to HIR.
fn hir_paren_to_hir(paren: &ExprParen) -> HirExpr {
    go_ast_expr_to_hir(&paren.expr)
}

/// Convert a Go array literal to HIR.
fn hir_array_to_hir(array: &ExprArray) -> HirExpr {
    let elems: Vec<HirExpr> = array.elems.iter().map(|e| go_ast_expr_to_hir(e)).collect();
    HirExpr::new(HirExprKind::Tuple(elems))
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
    // Range on its own is usually used in `for` loops
    HirExpr::new(HirExprKind::Slice {
        collection: Box::new(HirExpr::new(HirExprKind::Unsupported("range expression".to_string()))),
        start,
        end,
    })
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
fn hir_continue_to_hir(cont: &ExprContinue) -> HirExpr {
    HirExpr::new(HirExprKind::Block(HirBlock {
        stmts: vec![HirStatement::Continue],
    }))
}

/// Convert a Go return expression to HIR.
fn hir_return_to_hir(ret: &ExprReturn) -> HirExpr {
    let value = ret.expr.as_ref().map(|e| Box::new(go_ast_expr_to_hir(e)));
    HirExpr::new(HirExprKind::Block(HirBlock {
        stmts: vec![HirStatement::Return(value)],
    }))
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
    HirExpr::new(HirExprKind::Closure { params, body })
}

/// Convert a Go struct expression to HIR.
fn hir_struct_to_hir(_struct_expr: &syn::ExprStruct) -> HirExpr {
    // Struct literals are not supported in the basic HIR yet.
    HirExpr::new(HirExprKind::Unsupported("struct literal".to_string()))
}

/// Convert a Go macro expression to HIR.
fn hir_macro_to_hir(macro_expr: &syn::ExprMacro) -> HirExpr {
    // Pass through macro tokens (vec!, format!, etc.)
    HirExpr::new(HirExprKind::Macro(macro_expr.mac.tokens.clone()))
}

/// Convert a Go match expression to HIR.
fn hir_match_to_hir(_match_expr: &syn::ExprMatch) -> HirExpr {
    // Match expressions are not supported in the basic HIR yet.
    HirExpr::new(HirExprKind::Unsupported("match".to_string()))
}

/// Convert a Go type to HIR.
fn hir_type_from_syn(ty: &Type) -> Box<HirType> {
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
        Type::Tuple(type_tuple) => {
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
        syn::Stmt::Item(_) => {
            // Items inside blocks are not supported in the basic HIR.
            HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Unsupported("item in block".to_string()))))
        }
        _ => {
            HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Unsupported("unsupported statement".to_string()))))
        }
    }
}

/// Convert a Go AST statement to a HIR statement.
///
/// This is the core conversion function for GoStmt variants.
/// Each variant maps to the corresponding HIR statement type.
pub fn go_stmt_to_hir(stmt: &crate::transpiler::ast::GoStmt) -> HirStatement {
    use crate::transpiler::ast::GoStmt;
    use crate::transpiler::ast::GoBlock;

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
                            crate::transpiler::ast::GoForInit::Single(ident, _) => {
                                // `for i := range items` — only index, no value
                                HirStatement::ForRange {
                                    index_name: None,
                                    value_name: ident.clone(),
                                    iterable,
                                    body: go_block_to_hir(&go_for.body),
                                }
                            }
                            crate::transpiler::ast::GoForInit::Double(ident1, ident2, _) => {
                                // `for i, v := range items`
                                HirStatement::ForRange {
                                    index_name: Some(ident1.clone()),
                                    value_name: ident2.clone(),
                                    iterable,
                                    body: go_block_to_hir(&go_for.body),
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
                let init = go_for.init.as_ref().map(|go_for_init| {
                    match go_for_init {
                        crate::transpiler::ast::GoForInit::Single(ident, value) => {
                            // `i := 0` or `i = 0`
                            let _value = value.as_ref()
                                .map(|v| go_ast_expr_to_hir(v))
                                .unwrap_or_else(|| HirExpr::new(HirExprKind::Literal(HirLiteral::Int(0))));
                            let expr: syn::Expr = syn::parse_quote!(#ident = 0);
                            Box::new(go_ast_expr_to_hir(&expr))
                        }
                                        crate::transpiler::ast::GoForInit::Double(ident1, ident2, value) => {
                            // `i, v := 0, 0` (first index, second value)
                            let _value = value.as_ref()
                                .map(|v| go_ast_expr_to_hir(v))
                                .unwrap_or_else(|| HirExpr::new(HirExprKind::Literal(HirLiteral::Int(0))));
                            let expr: syn::Expr = syn::parse_quote!((#ident1, #ident2) = 0);
                            Box::new(go_ast_expr_to_hir(&expr))
                        }
                    }
                });

                let condition = go_for.cond
                    .as_ref()
                    .map(|e| Box::new(go_ast_expr_to_hir(e)))
                    .unwrap_or_else(|| Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Bool(true)))));

                let post = go_for.post
                    .as_ref()
                    .map(|e| Box::new(go_ast_expr_to_hir(e)))
                    .map(|e| {
                        // Handle `i++` as an assignment expression
                        e
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
            let selector = Box::new(
                sw.selector.as_ref()
                    .map(|e| go_ast_expr_to_hir(e))
                    .unwrap_or_else(|| HirExpr::new(HirExprKind::Literal(HirLiteral::Bool(true))))
            );

            // Build match arms
            let arms: Vec<(Box<HirExpr>, HirBlock)> = sw.cases.iter().map(|sc| {
                let patterns: Vec<Box<HirExpr>> = sc.exprs.iter().map(|e| {
                    Box::new(go_ast_expr_to_hir(e))
                }).collect();
                // Take the first pattern as the arm key
                let pattern = patterns.into_iter().next()
                    .unwrap_or_else(|| Box::new(HirExpr::new(HirExprKind::Unsupported("empty case".to_string()))));
                let stmts = sc.stmts.iter().map(|s| go_stmt_to_hir(s)).collect();
                let body = HirBlock { stmts };
                (pattern, body)
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
            // This is a Go-specific concurrency primitive
            let cases: Vec<(Box<HirExpr>, HirBlock)> = select_stmt.cases.iter().map(|case| {
                match case {
                    crate::transpiler::ast::GoSelectCase::Send { ch, value } => {
                        // Send case: `ch <- value`
                        let channel_expr: syn::Expr = syn::parse2(TokenStream::clone(ch)).unwrap_or_else(|_| syn::parse_quote!(0));
                        let value_expr: syn::Expr = syn::parse2(TokenStream::clone(value)).unwrap_or_else(|_| syn::parse_quote!(0));
                        let channel = Box::new(go_ast_expr_to_hir(&channel_expr));
                        let val = Box::new(go_ast_expr_to_hir(&value_expr));
                        let pattern = Box::new(HirExpr::new(HirExprKind::ChannelSend {
                            channel,
                            value: val,
                        }));
                        let body = HirBlock::default();
                        (pattern, body)
                    }
                    crate::transpiler::ast::GoSelectCase::Recv { ch, target: _ } => {
                        // Recv case: `<-ch`
                        let channel_expr: syn::Expr = syn::parse2(TokenStream::clone(ch)).unwrap_or_else(|_| syn::parse_quote!(0));
                        let channel = Box::new(go_ast_expr_to_hir(&channel_expr));
                        let pattern = Box::new(HirExpr::new(HirExprKind::ChannelRecv {
                            channel,
                            target: None,
                        }));
                        let body = HirBlock::default();
                        (pattern, body)
                    }
                    crate::transpiler::ast::GoSelectCase::Default(body) => {
                        // Default case
                        let pattern = Box::new(HirExpr::new(HirExprKind::Literal(HirLiteral::Nil)));
                        let stmts = body.stmts.iter().map(|s| go_stmt_to_hir(s)).collect();
                        let body = HirBlock { stmts };
                        (pattern, body)
                    }
                }
            }).collect();
            
            HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Select {
                cases,
                default_body: None,
            })))
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
                    syn::Stmt::Item(_) => HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Unsupported(
                        "item in defer body".to_string()
                    )))),
                    _ => HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Unsupported(
                        "unknown statement in defer".to_string()
                    )))),
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
                HirStatement::Expr(Box::new(HirExpr::new(HirExprKind::Unsupported(
                    "make statement: insufficient args".to_string()
                ))))
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
            let body = go_block_to_hir_with_stmts(body_stmts);
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
            let value_expr: syn::Expr = syn::parse2(value_stream.clone())
                .unwrap_or_else(|_| syn::parse_quote!(0));
            let value = Box::new(go_ast_expr_to_hir(&value_expr));
            HirStatement::Local {
                name: name.clone(),
                mutable: true,
                value,
            }
        }
        GoStmt::GoShortDecl(name, value_stream) => {
            // Go `:=` short declaration (non-closure)
            // This is like `x := value` but with raw TokenStream
            let value_expr: syn::Expr = syn::parse2(value_stream.clone())
                .unwrap_or_else(|_| syn::parse_quote!(0));
            let value = Box::new(go_ast_expr_to_hir(&value_expr));
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
    go_block_to_hir_with_stmts(&block.stmts)
}

/// Convert Go statements to a HIR block.
pub fn go_block_to_hir_with_stmts(stmts: &[GoStmt]) -> HirBlock {
    let body_stmts: Vec<HirStatement> = stmts.iter().map(|stm| go_stmt_to_hir(stm)).collect();
    HirBlock { stmts: body_stmts }
}

/// Check if a HIR expression is a simple identifier.
pub fn is_simple_identifier(expr: &HirExpr) -> bool {
    matches!(&expr.kind, HirExprKind::Identifier(_))
}

/// Get the name of a HIR identifier expression.
pub fn get_identifier_name(expr: &HirExpr) -> Option<&Ident> {
    match &expr.kind {
        HirExprKind::Identifier(id) => Some(id),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transpiler::ast::{GoBlock, GoStmt, GoForInit, GoIf, GoFor, GoSelect, GoSelectCase, GoWhile, Switch, SwitchCase, GoImport};
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
            HirStatement::Expr(expr) => {
                match expr.kind {
                    HirExprKind::Select { cases: c, default_body } => {
                        assert!(!c.is_empty());
                        assert!(default_body.is_none());
                    }
                    _ => panic!("Expected Select expression"),
                }
            }
            _ => panic!("Expected Expr statement"),
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


