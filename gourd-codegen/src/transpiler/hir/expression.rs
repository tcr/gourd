//! HIR Expression representation.
//!
//! Provides a clean, strongly-typed representation of Go expressions.
//! Expressions are the leaf nodes of the HIR — they capture the semantic
//! intent of Go expressions without token-level details.

use super::types::HirType;
use super::statement::HirBlock;

/// A wrapper around `syn::Path` that implements `Debug`.
///
/// `syn::Path` doesn't implement `Debug`, which breaks `#[derive(Debug)]`
/// on enums containing it. This wrapper solves that problem.
#[derive(Clone)]
pub struct HirPath(pub syn::Path);

impl std::fmt::Debug for HirPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = quote::quote!(#self.0).to_string();
        f.debug_tuple("HirPath").field(&s).finish()
    }
}

impl quote::ToTokens for HirPath {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.0.to_tokens(tokens);
    }
}

/// A HIR expression node.
///
/// This enum represents all possible Go expression forms. Each variant
/// captures the semantic structure directly, avoiding the token-level
/// bugs that plague the current transpiler.
#[derive(Clone)]
pub struct HirExpr {
    pub kind: HirExprKind,
}

/// The kinds of HIR expressions.
///
/// These mirror Go expression constructs, mapped to their Rust equivalents
/// where applicable. Each variant is exhaustive for the Go expressions
/// we support.
#[derive(Clone)]
pub enum HirExprKind {
    // Literals
    Literal(HirLiteral),

    // Identifiers (variables, function names, etc.)
    Identifier(syn::Ident),

    // Binary operations
    Binary {
        op: HirBinaryOp,
        lhs: Box<HirExpr>,
        rhs: Box<HirExpr>,
    },

    // Unary operations
    Unary {
        op: HirUnaryOp,
        operand: Box<HirExpr>,
    },

    // Function calls
    Call {
        func: Box<HirExpr>,
        args: Vec<HirExpr>,
    },

    // Method calls: `receiver.method(args)`
    MethodCall {
        receiver: Box<HirExpr>,
        method: syn::Ident,
        args: Vec<HirExpr>,
    },

    // Field access: `receiver.field`
    FieldAccess {
        receiver: Box<HirExpr>,
        field: syn::Ident,
    },

    // Index access: `expr[index]`
    Index {
        collection: Box<HirExpr>,
        index: Box<HirExpr>,
    },

    // Slicing: `expr[start:end]` (maps to `expr[start..end]` or `expr[start..]`)
    Slice {
        collection: Box<HirExpr>,
        start: Option<Box<HirExpr>>,
        end: Option<Box<HirExpr>>,
    },

    // Range-based iteration variable reference
    RangeVar(syn::Ident),

    // Go type conversion calls: `int(x)` → `(x as i32)`, `string(x)` → `String::from(x)`, etc.
    TypeConvert {
        func: syn::Ident,
        arg: Box<HirExpr>,
    },

    // Type cast: `x.(T)`
    Cast {
        value: Box<HirExpr>,
        target_type: Box<HirType>,
    },

    // Tuple (multi-return values)
    Tuple(Vec<HirExpr>),

    // Block expression
    Block(HirBlock),

    // Closure expression
    Closure {
        params: Vec<(syn::Ident, Option<Box<HirType>>)>,
        body: HirBlock,
    },

    // Error handling: `if err != nil` check
    ErrorCheck {
        value: Box<HirExpr>,
    },

    // Special builtin operations
    Len(Box<HirExpr>),          // `len(x)` → `x.len() as i32`
    Cap(Box<HirExpr>),          // `cap(x)` → `x.capacity() as i32`
    Make(MakeKind),             // `make(...)`
    Append {
        target: Box<HirExpr>,
        elements: Vec<HirExpr>,
    },
    Copy {
        dst: Box<HirExpr>,
        src: Box<HirExpr>,
    },
    /// Full path expression: `::gourd::prelude::fields` or `strings.Join`
    Path(HirPath),
    /// Macro invocation: `vec![...]`, `format!(...)`
    Macro(proc_macro2::TokenStream),
    /// Unsupported/placeholder — represents a Go construct that the HIR
    /// does not yet support. Used during development for gradual integration.
    Unsupported(String),
    /// Slice literal: `[]T{elem1, elem2, ...}`
    SliceLiteral(Vec<HirExpr>),
    /// Map literal: `map[K]V{key1: val1, key2: val2, ...}`
    Map(Vec<(Box<HirExpr>, Box<HirExpr>)>),
    /// Channel send: `ch <- value`
    ChannelSend {
        channel: Box<HirExpr>,
        value: Box<HirExpr>,
    },
    /// Channel receive: `<-ch`
    ChannelRecv {
        channel: Box<HirExpr>,
        target: Option<syn::Ident>,
    },
    /// Select statement: `select { case ... default: ... }`
    Select {
        cases: Vec<(Box<HirExpr>, HirBlock)>,
        default_body: Option<HirBlock>,
    },
    /// Match expression: `match selector { arm1, arm2, ... }`
    Match {
        selector: Box<HirExpr>,
        arms: Vec<(Box<HirExpr>, HirBlock)>,
        default_body: Option<HirBlock>,
    },
}

/// A HIR literal value.
#[derive(Debug, Clone)]
pub enum HirLiteral {
    Int(i64),
    Float(f64),
    Bool(bool),
    StringTy(String),
    Nil,
}

/// Binary operators in HIR.
///
/// Maps Go operators to their Rust equivalents. The key insight is that
/// operators like `+` have different meanings for strings vs. numbers.
/// The HIR captures the semantic intent, letting the codegen phase
/// handle the details.
#[derive(Debug, Clone)]
pub enum HirBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Xor,
    AndNot,
    BitAnd,
    BitOr,
    BitXor,
    Lsh,
    Rsh,
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
    AndAssign,
    OrAssign,
}

/// Unary operators in HIR.
#[derive(Debug, Clone)]
pub enum HirUnaryOp {
    Not,      // `!x` — logical NOT (lower precedence than `<` in Go)
    Neg,      // `-x` — negation
    Deref,    // `*x` — dereference
    AddressOf, // `&x` — address of
}

/// The kinds of `make()` operations.
#[derive(Clone)]
pub enum MakeKind {
    Slice(Box<HirType>, Box<HirExpr>),   // `make([]T, len)`
    SliceWithCap(Box<HirType>, Box<HirExpr>, Box<HirExpr>), // `make([]T, len, cap)`
    Map(Box<HirType>, Box<HirType>),     // `make(map[K]V)`
    MapWithCap(Box<HirType>, Box<HirType>, Box<HirExpr>),    // `make(map[K]V, cap)`
    Channel(Box<HirType>),               // `chan T`
    ChannelWithCap(Box<HirType>, Box<HirExpr>), // `chan T{cap}`
}

impl HirExpr {
    /// Create a new HIR expression.
    pub fn new(kind: HirExprKind) -> Self {
        HirExpr { kind }
    }

    /// Check if this expression is a simple identifier.
    pub fn is_simple_identifier(&self) -> bool {
        matches!(&self.kind, HirExprKind::Identifier(_))
    }

    /// Get the identifier name if this is an identifier expression.
    pub fn as_identifier(&self) -> Option<&syn::Ident> {
        match &self.kind {
            HirExprKind::Identifier(id) => Some(id),
            _ => None,
        }
    }

    /// Check if this expression is a simple identifier matching a name.
    pub fn is_identifier_named(&self, name: &str) -> bool {
        match &self.kind {
            HirExprKind::Identifier(id) => id == name,
            _ => false,
        }
    }

    /// Check if this expression is a numeric literal.
    pub fn is_numeric_literal(&self) -> bool {
        matches!(&self.kind, HirExprKind::Literal(HirLiteral::Int(_)))
    }
}

impl HirLiteral {
    /// Check if this is an integer literal.
    pub fn is_int(&self) -> bool {
        matches!(self, HirLiteral::Int(_))
    }

    /// Check if this is a string literal.
    pub fn is_string(&self) -> bool {
        matches!(self, HirLiteral::StringTy(_))
    }

    /// Get the integer value if this is an int literal.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            HirLiteral::Int(n) => Some(*n),
            _ => None,
        }
    }

    /// Get the string value if this is a string literal.
    pub fn as_string(&self) -> Option<&str> {
        match self {
            HirLiteral::StringTy(s) => Some(s),
            _ => None,
        }
    }
}

impl HirExprKind {
    /// Extract the block from a Block variant, or panic.
    pub fn unwrap_block(self) -> HirBlock {
        match self {
            HirExprKind::Block(block) => block,
            _ => panic!("unwrap_block called on non-Block HirExprKind"),
        }
    }
}
