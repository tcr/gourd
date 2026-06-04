//! Go source AST types — type definitions only, no parsing logic.

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::parse::{Parse, ParseStream};
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
    Select(GoSelect), // `select { ... }`
    Defer(TokenStream), // `defer func() { ... }` - runs at end of scope
    GoIfErr(TokenStream, Vec<GoStmt>), // `if err != nil { ... }` error handling
}

/// Select statement: `select { case ... default: ... }`.
pub(crate) struct GoSelect {
    pub(crate) cases: Vec<GoSelectCase>,
}

/// A single case inside a select statement.
pub(crate) enum GoSelectCase {
    /// Send case: `ch <- value`
    Send { ch: Box<TokenStream>, value: Box<TokenStream> },
    /// Recv case: `<-ch` or `x := <-ch`
    Recv { ch: Box<TokenStream>, target: Option<Ident> },
    /// Default case: `default: ...`
    Default(GoBlock),
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
#[derive(Default)]
pub(crate) struct GoBlock {
    pub(crate) stmts: Vec<GoStmt>,
}

impl Parse for GoBlock {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let block_content;
        syn::braced!(block_content in input);
        let mut stmts = Vec::new();
        while !block_content.is_empty() {
            stmts.push(block_content.parse::<GoStmt>()?);
        }
        Ok(GoBlock { stmts })
    }
}

impl ToTokens for GoBlock {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let body: Vec<_> = self.stmts.iter().map(|s| s.to_token_stream()).collect();
        proc_macro2::TokenStream::from(quote::quote! { #(#body);* }).to_tokens(tokens);
    }
}

impl Parse for GoIf {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let cond: Expr = input.parse()?;
        let then_block: GoBlock = input.parse()?;
        let else_block = if input.peek(syn::token::Else) {
            let _: syn::token::Else = input.parse()?;
            if input.peek(syn::token::If) {
                // else if — parse as else block
                Some(input.parse()?)
            } else {
                Some(input.parse()?)
            }
        } else {
            None
        };
        Ok(GoIf { cond, then_block, else_block })
    }
}

impl Parse for GoWhile {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _: syn::Ident = input.parse()?; // consume 'while'
        let cond: Expr = input.parse()?;
        let body: GoBlock = input.parse()?;
        Ok(GoWhile { cond, body })
    }
}

impl Parse for GoForInit {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let first: Ident = input.parse()?;
        if input.peek(syn::token::Comma) {
            let _: syn::token::Comma = input.parse()?;
            let second: Ident = input.parse()?;
            Ok(GoForInit::Double(first, second))
        } else {
            Ok(GoForInit::Single(first))
        }
    }
}

impl Parse for GoFor {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _: syn::token::For = input.parse()?;

        // Parse optional init
        let init = if input.peek(syn::Ident) {
            let fork = input.fork();
            if let Ok(first_ident) = fork.parse::<syn::Ident>() {
                if first_ident.to_string() == "range" {
                    None
                } else {
                    input.parse::<syn::Ident>()?;
                    if input.peek(syn::token::Comma) {
                        let _: syn::token::Comma = input.parse()?;
                        let second_ident = input.parse::<syn::Ident>()?;
                        let _: syn::token::Colon = input.parse()?;
                        let _: syn::token::Eq = input.parse()?;
                        Some(GoForInit::Double(first_ident, second_ident))
                    } else {
                        let _: syn::token::Colon = input.parse()?;
                        let _: syn::token::Eq = input.parse()?;
                        Some(GoForInit::Single(first_ident))
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        // Consume 'range' keyword
        if input.peek(syn::Ident) {
            let fork = input.fork();
            if let Ok(range_kw) = fork.parse::<syn::Ident>() {
                if range_kw.to_string() == "range" {
                    let _: syn::Ident = input.parse()?;
                }
            }
        }

        let iterable: syn::Path = input.parse()?;
        let body: GoBlock = input.parse()?;

        Ok(GoFor {
            init,
            is_range: true,
            iterable,
            body,
        })
    }
}

impl Parse for GoStmt {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        use syn::parse::discouraged::Speculative;

        // 1. Go short variable declaration with `:=` — also check for closures
        // 2. Rust `let` local declarations
        let fork = input.fork();
        if fork.peek(syn::token::Let) || fork.peek2(syn::token::Colon) {
            // Rust `let` statements or Go `:=` short declarations
            if fork.peek(syn::token::Let) {
                // Rust `let` — parse normally
                if let Ok(stmt) = fork.parse::<syn::Stmt>() {
                    if let syn::Stmt::Local(local) = stmt {
                        input.parse::<syn::Stmt>()?;
                        return Ok(GoStmt::Local(local));
                    }
                }
            }

            // Go `:=` short declaration — check if it's a closure
            // Pattern: `name := func(params) { body }`
            let check_fork = input.fork();
            if check_fork.peek2(syn::token::Colon) {
                // Skip past `name := `
                let _ = check_fork.parse::<proc_macro2::TokenTree>(); // name
                let _ = check_fork.parse::<syn::token::Colon>();
                let _ = check_fork.parse::<syn::token::Eq>();
                // Check if next token is `func`
                if check_fork.peek(syn::Ident) {
                    if let Ok(func_id) = check_fork.parse::<syn::Ident>() {
                        if func_id.to_string() == "func" {
                            // This is a Go closure! Parse the full assignment.
                            let local_tokens: TokenStream = input.parse()?;
                            // Extract the variable name
                            let pat_ident: Ident = local_tokens
                                .clone()
                                .into_iter()
                                .filter_map(|t| {
                                    if let proc_macro2::TokenTree::Ident(i) = t {
                                        Some(i)
                                    } else {
                                        None
                                    }
                                })
                                .next()
                                .unwrap_or_else(|| Ident::new("_", proc_macro2::Span::call_site()));
                            // Parse the closure (skip the `name := ` prefix)
                            let skip = input.fork();
                            let _ = skip.parse::<proc_macro2::TokenTree>(); // name
                            let _ = skip.parse::<syn::token::Colon>();
                            let _ = skip.parse::<syn::token::Eq>();
                            let closure_tokens: TokenStream = skip.parse().unwrap_or_default();
                            if let Some(closure_expr) = try_parse_closure_from_input(&closure_tokens) {
                                return Ok(GoStmt::GoLocal(pat_ident, closure_expr));
                            }
                        }
                    }
                }
            }

            // Not a closure — fall through to try parsing as regular expression
        }

        // 2. Check for if statement (accept both Rust keyword and Go identifier)
        if input.peek(syn::token::If) {
            let _if: syn::token::If = input.parse()?;
            let result: GoIf = input.parse()?;
            return Ok(GoStmt::If(result));
        }
        if input.peek(syn::Ident) {
            let fork = input.fork();
            if let Ok(kw) = fork.parse::<syn::Ident>() {
                if kw.to_string() == "if" {
                    // Consume the `if` identifier first (Go identifier)
                    let _if: syn::Ident = input.parse()?;
                    let result: GoIf = input.parse()?;
                    return Ok(GoStmt::If(result));
                }
            }
        }

        // 3. Check for while/for/switch/select/continue
        if input.peek(syn::Ident) {
            let fork = input.fork();
            if let Ok(kw) = fork.parse::<syn::Ident>() {
                let kw_str = kw.to_string();
                if kw_str == "while" {
                    let result: GoWhile = input.parse()?;
                    return Ok(GoStmt::While(result));
                }
                if kw_str == "for" {
                    let result: GoFor = input.parse()?;
                    return Ok(GoStmt::GoFor(result));
                }
                if kw_str == "switch" {
                    let result: Switch = input.parse()?;
                    return Ok(GoStmt::Switch(result));
                }
                if kw_str == "select" {
                    let result: GoSelect = input.parse()?;
                    return Ok(GoStmt::Select(result));
                }
                if kw_str == "continue" {
                    return Ok(GoStmt::Continue);
                }
            }
        }

        // 4. Check for channel send: `ch <- value`
        if input.peek(syn::token::Lt) || input.peek(syn::token::Le) {
            let chan_expr: Expr = input.parse()?;
            // Consume `<-` or `<=`
            if input.peek(syn::token::Lt) {
                let _: proc_macro2::Punct = input.parse()?;
                if input.peek(syn::token::Lt) || input.peek(syn::token::Le) {
                    let _: proc_macro2::Punct = input.parse()?;
                }
            } else if input.peek(syn::token::Le) {
                let _: proc_macro2::Punct = input.parse()?;
            }
            let val_expr: Expr = input.parse()?;
            return Ok(GoStmt::GoChannelSend(chan_expr, val_expr));
        }

        // 5. Check for channel recv: `<-ch`
        if input.peek(syn::token::Lt) || input.peek(syn::token::Le) {
            if input.peek(syn::token::Lt) {
                let _: proc_macro2::Punct = input.parse()?;
                if input.peek(syn::token::Lt) || input.peek(syn::token::Le) {
                    let _: proc_macro2::Punct = input.parse()?;
                }
            } else if input.peek(syn::token::Le) {
                let _: proc_macro2::Punct = input.parse()?;
            }
            let ch_expr: Expr = input.parse()?;
            return Ok(GoStmt::GoChannelRecv(ch_expr));
        }

        // 6. Check for return
        if input.peek(syn::token::Return) {
            let fork = input.fork();
            if let Ok(_) = fork.parse::<syn::token::Return>() {
                let _ret: syn::token::Return = input.parse()?;
                if input.is_empty() || input.peek(syn::token::Semi) || input.peek(syn::token::Colon) {
                    return Ok(GoStmt::GoReturn(vec![]));
                }
                let val: Expr = input.parse()?;
                return Ok(GoStmt::GoReturn(vec![val]));
            }
        }

        // 7. Try expression parsing
        let expr_fork = input.fork();
        if let Ok(_expr) = expr_fork.parse::<Expr>() {
            input.advance_to(&expr_fork);
            let expr: Expr = input.parse()?;
            return Ok(GoStmt::Expr(expr));
        }

        // 7b. If expression parsing failed, check if the raw tokens contain
        // a Go closure (anonymous function). This happens when syn can't
        // parse Go syntax like `func(params) { body }` as a Rust expression.
        let input_tokens: TokenStream = input.parse().unwrap_or_default();
        if let Some(closure_expr) = try_parse_closure_from_input(&input_tokens) {
            return Ok(GoStmt::RawStmt(closure_expr));
        }

        // 8. Fallback: skip one token to make progress
        let _ = input.parse::<proc_macro2::TokenTree>();
        Ok(GoStmt::RawStmt(proc_macro2::TokenStream::new()))
    }
}

impl ToTokens for GoStmt {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use super::stmt_to_rust::go_stmt_to_rust;
        let rust = go_stmt_to_rust(self);
        rust.to_tokens(tokens);
    }
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

/// Try to parse a Go closure from raw tokens.
/// Returns a TokenStream with the Rust closure syntax if successful.
pub(crate) fn try_parse_closure_from_input(tokens: &TokenStream) -> Option<TokenStream> {
    let trees: Vec<proc_macro2::TokenTree> = tokens.clone().into_iter().collect();

    // Must start with `func` keyword
    if trees.is_empty() {
        return None;
    }
    if let proc_macro2::TokenTree::Ident(id) = &trees[0] {
        if id.to_string() != "func" {
            return None;
        }
    } else {
        return None;
    }

    // Pass the full closure to go_to_rust_closure which handles all parsing internally
    let closure_expr = super::free_fn::go_to_rust_closure(tokens.clone());
    Some(closure_expr)
}

