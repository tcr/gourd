//! High-level intermediate representation for Go → Rust transpilation.
//!
//! The HIR is a clean, strongly-typed semantic representation of Go code
//! that sits between parsing (Go AST) and code generation (Rust tokens).
//! This architecture eliminates the brittle token-level manipulation that
//! caused operator precedence bugs and made debugging difficult.
//!
//! ## Architecture
//!
//! ```ignore
//! Go source tokens --parse--> Go AST (ast.rs) --convert--> HIR (hir/) --codegen--> Rust TokenStream
//! ```
//!
//! ### Why HIR?
//!
//! The current approach transpiles Go AST → Rust tokens directly using `quote!`.
//! This is brittle because:
//! - Operators have different precedence in Go vs. Rust (e.g., `!` binds tighter
//!   than `<` in Rust but looser in Go)
//! - Type information is lost when stored as `TokenStream`
//! - Debugging requires tracing token streams instead of semantic data
//! - There's no validation layer between parsing and codegen
//!
//! The HIR fixes this by:
//! 1. Capturing **semantic intent** (not token text)
//! 2. Providing **strong type safety** (can't represent invalid Go constructs)
//! 3. Separating **analysis** from **code generation**
//! 4. Being independently **testable and debuggable**
//!
//! ### Current Phase
//!
//! The HIR is under active development. Currently:
//! - ✅ `hir/types.rs` — Type system (complete)
//! - ✅ `hir/expression.rs` — Expression tree (complete)
//! - ✅ `hir/statement.rs` — Statement tree (complete)
//! - ✅ `hir/conversion.rs` — Go AST → HIR conversion (complete)
//! - ✅ `hir/codegen.rs` — HIR → Rust token generation (complete)
//! - ⬜ `hir/control_flow.rs` — Control flow HIR (planned)
//! - ⬜ `hir/declaration.rs` — Function/struct declarations (planned)
//!
//! The conversion and codegen modules are integrated. See `hir/conversion.rs`
//! for the Go AST → HIR bridge, and `hir/codegen.rs` for HIR → Rust tokens.

// Go AST types — moved from legacy transpiler/ast.rs
pub mod ast;
// Go type name mapping — moved from legacy transpiler/types.rs  
pub mod types;
pub mod expression;
pub mod statement;
pub mod conversion;
pub mod codegen;

// Re-export key types for downstream use
pub use types::{ HirType, HirTypeKind };
pub use expression::{ HirExpr, HirExprKind, HirLiteral };
pub use statement::{ HirStatement, HirBlock };
pub use conversion::{ go_ast_expr_to_hir, go_select_to_hir, is_simple_identifier, get_identifier_name, go_switch_to_hir };
pub use codegen::{ hir_expr_to_rust, hir_select_to_rust_from_hir, hir_stmt_to_rust,
                   go_to_rust_closure_hir, go_to_rust_fn_hir, go_to_rust_interface_hir, go_to_rust_receiver_fn_hir,
                   go_to_rust_select_hir, go_to_rust_struct_hir, go_to_rust_switch_hir,
                   switch_to_rust };

/// A complete HIR for a Go function.
/// This is the top-level unit of HIR for free functions.
pub struct HirFunction {
    /// Function name
    pub name: syn::Ident,
    /// Parameter names and types
    pub params: Vec<(syn::Ident, Box<HirType>)>,
    /// Return types (Go allows multiple)
    pub returns: Vec<Box<HirType>>,
    /// Function body statements
    pub body: HirBlock,
}
