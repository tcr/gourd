//! HIR Statement representation.
//!
/// Statements capture semantic intent, not token text. This module provides
/// a clean, strongly-typed representation of Go statements for use in
/// the HIR.

use super::expression::{ HirExpr, HirExprKind };
use super::types::{ HirType };
use proc_macro2::TokenStream;

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
        init: Option<Box<HirExpr>>,
        condition: Box<HirExpr>,
        post: Option<Box<HirExpr>>,
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
