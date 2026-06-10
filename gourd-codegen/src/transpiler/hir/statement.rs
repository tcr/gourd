//! HIR Statement representation.
//!
/// Statements capture semantic intent, not token text. This module provides
/// a clean, strongly-typed representation of Go statements for use in
/// the HIR.

use super::expression::{ HirExpr, HirExprKind };
use super::types::{ HirType };
use proc_macro2::TokenStream;
use syn::Expr;
use syn::parse::ParseStream;

/// A HIR statement (higher-level than Go AST statements).
///
/// Unlike `GoStmt` in `ast.rs`, which stores raw tokens in many variants,
/// this captures the semantic intent directly.
#[derive(Clone)]
pub enum HirStatement {
    /// Local variable declaration: `x := value` or `let x = value`
    Local {
        name: syn::Ident,
        mutable: bool,
        value: Box<HirExpr>,
    },
    /// Assignment: `x = value`
    Assign {
        target: Box<HirExpr>,
        value: Box<HirExpr>,
    },
    /// Expression statement: `foo(x)` (side-effect only, result discarded)
    Expr(Box<HirExpr>),
    /// If/else: `if cond { body } else { body }`
    If {
        cond: Box<HirExpr>,
        then_body: HirBlock,
        else_body: Option<HirBlock>,
    },
    /// While loop: `while cond { body }`
    While {
        cond: Box<HirExpr>,
        body: HirBlock,
    },
    /// Range-based iteration: `for _, v := range items { body }`
    ForRange {
        index_name: Option<syn::Ident>,
        value_name: syn::Ident,
        iterable: Box<HirExpr>,
        body: HirBlock,
    },
    /// C-style loop with init, condition, and post: `for i := 0; i < n; i++ { body }`
    ForLoop {
        init: Option<Box<HirStatement>>,
        condition: Box<HirExpr>,
        post: Option<Box<HirStatement>>,
        body: HirBlock,
    },
    /// Return statement: `return value` or `return` (no value)
    Return(Option<Box<HirExpr>>),
    /// Continue statement
    Continue,
    /// Break statement (with optional label)
    Break(Option<syn::Ident>),
    /// Channel send: `ch <- value`
    ChannelSend {
        channel: Box<HirExpr>,
        value: Box<HirExpr>,
    },
    /// Channel receive: `x := <-ch`
    ChannelRecv {
        channel: Box<HirExpr>,
        target: Option<syn::Ident>,
    },
    /// Type assertion: `x.(T)`
    TypeAssert {
        value: Box<HirExpr>,
        target_type: Box<HirType>,
        result_name: Option<syn::Ident>,
    },
    /// Short declaration with a closure: `f := func() { body }`
    Closure {
        name: syn::Ident,
        params: Vec<(syn::Ident, Option<Box<HirType>>)>,
        body: HirBlock,
    },
    /// Defer statement: `defer func() { body }`
    Defer {
        body: HirBlock,
    },
    /// Import declaration: `import s "strings"`, `import . "fmt"`, `import _ "os"`
    Import {
        alias: Option<String>,
        path: String,
        dot: bool,
        blank: bool,
    },
    /// Raw token stream — fallback for unknown/unparseable statements
    RawStmt {
        tokens: TokenStream,
    },
    /// Pre-transpiled switch return — fallback for complex switch expressions
    SwitchReturn {
        tokens: TokenStream,
    },
}

/// A block of HIR statements.
#[derive(Default, Clone)]
pub struct HirBlock {
    pub stmts: Vec<HirStatement>,
}

impl HirBlock {
    /// Create a new empty block.
    pub fn new() -> Self {
        HirBlock { stmts: Vec::new() }
    }

    /// Add a statement to this block.
    pub fn push(&mut self, stmt: HirStatement) {
        self.stmts.push(stmt);
    }

    /// Check if this block is empty.
    pub fn is_empty(&self) -> bool {
        self.stmts.is_empty()
    }

    /// Get the number of statements in this block.
    pub fn len(&self) -> usize {
        self.stmts.len()
    }
}

// ─── Go block parsing (moved from legacy stmts.rs) ────────────────────────────

/// Parse a Go block of statements.
pub(crate) fn parse_go_block(input: ParseStream) -> syn::Result<super::ast::GoBlock> {
    use super::ast::GoBlock;

    let brace_content = if input.peek(syn::token::Brace) {
        // Standard case: `{` punctuation
        let content;
        let _brace = syn::braced!(content in input);
        content
    } else {
        // Handle Group token with Brace delimiter
        let tt: proc_macro2::TokenTree = input.parse()?;
        match tt {
            proc_macro2::TokenTree::Group(g) if g.delimiter() == proc_macro2::Delimiter::Brace => {
                // Parse the body from the Group's inner TokenStream
                return parse_body_from_group(&g.stream());
            }
            _ => {
                return Err(input.error("expected body `{`"));
            }
        }
    };

    let mut stmts: Vec<super::ast::GoStmt> = Vec::new();
    while !brace_content.is_empty() {
        // Try special statements first (return, if, for, etc.)
        // Use unsafe transmute since HIR and legacy GoStmt are structurally identical
        let legacy_stmts: &mut Vec<crate::transpiler::hir::ast::GoStmt> =
            unsafe { std::mem::transmute(&mut stmts) };
        if crate::transpiler::legacy::stmts::parse_go_special_stmt(&brace_content, legacy_stmts)? {
            continue;
        }
        // Try base statements (local declarations, assignments, etc.)
        let legacy_stmts2: &mut Vec<crate::transpiler::hir::ast::GoStmt> =
            unsafe { std::mem::transmute(&mut stmts) };
        if let Ok(()) = crate::transpiler::legacy::base_stmts::parse_base_stmt(&brace_content, legacy_stmts2) {
            continue;
        }
        // Try to parse as a base statement (simple expressions, assignments, etc.)
        let token_stream: TokenStream = brace_content
            .cursor()
            .token_stream();
        let expr_result: Result<Expr, _> = syn::parse2::<Expr>(token_stream);
        if let Ok(expr) = expr_result {
            stmts.push(super::ast::GoStmt::Expr(expr));
        } else {
            // Skip unknown tokens to avoid infinite loops
            let _: proc_macro2::TokenTree = brace_content.parse()?;
        }
    }

    Ok(GoBlock { stmts })
}

/// Parse body from a Group's TokenStream.
pub(crate) fn parse_body_from_group(ts: &proc_macro2::TokenStream) -> syn::Result<super::ast::GoBlock> {
    use super::ast::GoBlock;

    let trees: Vec<proc_macro2::TokenTree> = ts.clone().into_iter().collect();
    let mut stmts = Vec::new();
    let mut i = 0;

    while i < trees.len() {
        if let proc_macro2::TokenTree::Group(g) = &trees[i] {
            if g.delimiter() == proc_macro2::Delimiter::Brace {
                // Parse the body from this group
                let inner_ts: proc_macro2::TokenStream = g.stream();
                let body = parse_body_from_group(&inner_ts)?;
                stmts.extend(body.stmts);
                i += 1;
            } else {
                i += 1;
            }
        } else if let proc_macro2::TokenTree::Ident(id) = &trees[i] {
            let id_str = id.to_string();

            // Handle if statement
            if id_str == "if" {
                if i + 1 < trees.len() {
                    if let proc_macro2::TokenTree::Group(g) = &trees[i + 1] {
                        if g.delimiter() == proc_macro2::Delimiter::Parenthesis {
                            let cond: syn::Expr = syn::parse2(g.stream())?;
                            i += 2;

                            // Parse then block
                            if i < trees.len() {
                                if let proc_macro2::TokenTree::Group(g) = &trees[i] {
                                    if g.delimiter() == proc_macro2::Delimiter::Brace {
                                        let body = parse_body_from_group(&g.stream())?;
                                        stmts.push(super::ast::GoStmt::If(super::ast::GoIf {
                                            cond,
                                            then_block: body,
                                            else_block: None,
                                        }));
                                        i += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            } else if id_str == "for" {
                // Parse for/while loop
                if i + 1 < trees.len() {
                    if let proc_macro2::TokenTree::Group(g) = &trees[i + 1] {
                        if g.delimiter() == proc_macro2::Delimiter::Parenthesis {
                            let cond: syn::Expr = syn::parse2(g.stream())?;
                            i += 2;

                            // Parse loop body
                            if i < trees.len() {
                                if let proc_macro2::TokenTree::Group(g) = &trees[i] {
                                    if g.delimiter() == proc_macro2::Delimiter::Brace {
                                        let body = parse_body_from_group(&g.stream())?;
                                        stmts.push(super::ast::GoStmt::While(super::ast::GoWhile {
                                            cond,
                                            body,
                                        }));
                                        i += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                // Try to parse as a base statement
                let mut stmt_stream = proc_macro2::TokenStream::new();
                while i < trees.len() {
                    if let proc_macro2::TokenTree::Punct(p) = &trees[i] {
                        if p.as_char() == ';' || p.as_char() == ',' {
                            i += 1;
                            break;
                        }
                    }
                    stmt_stream.extend(std::iter::once(trees[i].clone()));
                    i += 1;
                }
                if !stmt_stream.is_empty() {
                    stmts.push(super::ast::GoStmt::Expr(syn::parse2(stmt_stream)?));
                }
            }
        } else {
            // Try to parse reserved keyword as identifier (e.g., `return`)
            let kw_str = trees[i].to_string();
            if kw_str == "return" {
                // Parse return statement: collect tokens until semicolon
                i += 1;
                let mut expr_stream = proc_macro2::TokenStream::new();
                while i < trees.len() {
                    if let proc_macro2::TokenTree::Punct(p) = &trees[i] {
                        if p.as_char() == ';' {
                            i += 1;
                            break;
                        }
                    }
                    expr_stream.extend(std::iter::once(trees[i].clone()));
                    i += 1;
                }
                if !expr_stream.is_empty() {
                    if let Ok(expr) = syn::parse2::<Expr>(expr_stream.clone()) {
                        stmts.push(super::ast::GoStmt::GoReturn(vec![expr]));
                    } else {
                        stmts.push(super::ast::GoStmt::RawStmt(expr_stream));
                    }
                } else {
                    stmts.push(super::ast::GoStmt::GoReturn(vec![]));
                }
            } else {
                i += 1;
            }
        }
    }

    Ok(GoBlock { stmts })
}
