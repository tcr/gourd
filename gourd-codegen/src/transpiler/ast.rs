//! Go source AST types — type definitions only, no parsing logic.

use proc_macro2::TokenStream;
use syn::{Expr, Ident, Token};

/// Statement kinds used by the Go parser.
pub(crate) enum GoStmt {
    Local(syn::Local),
    GoLocal(Ident, TokenStream),
    If(GoIf),
    Expr(syn::Expr),
    GoSlice(Vec<Expr>),
    GoMap(String, Option<Box<syn::Type>>, Option<Box<syn::Type>>, Vec<(Expr, Expr)>),
    GoReturn(Vec<Expr>),  // multi-return: `return a, b`
    Switch(Switch),
    Continue,
    While(GoWhile),
    GoFor(GoFor),
    GoChannelSend(Expr, Expr), // `ch <- value`
    GoChannelRecv(Expr),       // `<- ch`
    GoTypeAssert(Expr, syn::Type), // `x.(T)` type assertion
    GoMake(String),   // `make(...)` with raw argument string
    RawStmt(TokenStream),
}

/// Loop with range/for classification.
pub(crate) struct GoFor {
    /// Optional init (e.g., `i := 0` or `i, v := `)
    pub(crate) init: Option<GoForInit>,
    /// Always true for `for` with `range`
    pub(crate) is_range: bool,
    /// The iterable expression (parsed as Path)
    pub(crate) iterable: syn::Path,
    /// The loop body
    pub(crate) body: GoBlock,
}

pub(crate) enum GoForInit {
    /// Single variable: `for i := range slice`
    Single(Ident),
    /// Two variables: `for i, v := range slice`
    Double(Ident, Ident),
}

/// While loop.
pub(crate) struct GoWhile {
    pub(crate) cond: Expr,
    pub(crate) body: GoBlock,
}

/// If/else statement.
pub(crate) struct GoIf {
    pub(crate) cond: Expr,
    pub(crate) then_block: GoBlock,
    pub(crate) else_block: Option<GoBlock>,
}

/// Block of statements inside a function, if, etc.
pub(crate) struct GoBlock {
    pub(crate) stmts: Vec<GoStmt>,
}

/// Function parameters (with shorthand grouping support).
pub(crate) struct GoFnInputs {
    pub(crate) args: Vec<GoParam>,
}

pub(crate) struct GoParam {
    pub(crate) id: Ident,
    pub(crate) ty: Option<Box<syn::Type>>,
    pub(crate) slice_elem: Option<syn::Type>,
}

/// Function return type.
pub(crate) struct GoFnOutput {
    pub(crate) tys: Vec<syn::Type>,
    pub(crate) is_slice: bool,
    pub(crate) elem_type: Option<Box<syn::Type>>,
}

/// Full function declaration.
pub(crate) struct GoFn {
    pub(crate) ident: Ident,
    pub(crate) generics: syn::punctuated::Punctuated<syn::GenericParam, Token![,]>,
    pub(crate) inputs: GoFnInputs,
    pub(crate) output: Option<GoFnOutput>,
    pub(crate) block: GoBlock,
}

/// Struct declaration.
pub(crate) struct GoStruct {
    pub(crate) ident: Ident,
    pub(crate) fields: Vec<GoStructField>,
}

pub(crate) struct GoStructField {
    pub(crate) name: Ident,
    pub(crate) ty: syn::Type,
}

/// Interface declaration: `interface Foo { Method() int }`.
pub(crate) struct GoInterface {
    pub(crate) ident: Ident,
    pub(crate) methods: Vec<GoInterfaceMethod>,
}

pub(crate) struct GoInterfaceMethod {
    pub(crate) name: Ident,
    pub(crate) inputs: GoFnInputs,
    pub(crate) output: Option<GoFnOutput>,
}

/// Switch statement with cases and default.
pub(crate) struct Switch {
    pub(crate) selector: Option<Expr>,
    pub(crate) cases: Vec<SwitchCase>,
    pub(crate) default_stmts: Vec<GoStmt>,
}

pub(crate) struct SwitchCase {
    pub(crate) exprs: Vec<Expr>,
    pub(crate) stmts: Vec<GoStmt>,
}
