//! Go source AST types — type definitions only, no parsing logic.

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::ext::IdentExt;
use syn::parse::{discouraged::Speculative, Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token;
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
    SwitchReturn(TokenStream),  // `return switch ...` with pre-transpiled match
    Select(GoSelect), // `select { ... }`
    Defer(TokenStream), // `defer func() { ... }` - runs at end of scope
    GoIfErr(TokenStream, Vec<GoStmt>), // `if err != nil { ... }` error handling
    GoImport(GoImport), // `import "strings"` — go package import declaration
    GoShortDecl(Ident, TokenStream), // Go `:=` short declaration (non-closure)
}

/// Go import declaration: `import "strings"`, `import s "strings"`, `import . "fmt"`, `import _ "os"`.
pub(crate) struct GoImport {
    pub(crate) alias: Option<Ident>,   // None for default package name, Some("s") for `import s ...`
    pub(crate) dot: bool,              // `import . "fmt"` — makes all names visible
    pub(crate) blank: bool,            // `import _ "os"` — side-effect only, no Rust output
    pub(crate) path: String,           // path string, e.g. `"strings"`, `"golang.org/x/sys/unix"`
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
    Recv { ch: Box<TokenStream>, #[allow(dead_code)] target: Option<Ident> },
    /// Default case: `default: ...`
    Default(GoBlock),
}

/// Loop with range/for classification.
pub(crate) struct GoFor {
    /// Optional init (e.g., `i := 0` or `i, v := `)
    pub(crate) init: Option<GoForInit>,
    /// True for `for` with `range`, false for C-style `for`
    pub(crate) is_range: bool,
    /// The iterable expression (range only, parsed as Path)
    pub(crate) iterable: Option<syn::Path>,
    /// The loop condition (C-style only, None for range)
    pub(crate) cond: Option<Box<syn::Expr>>,
    /// The post statement (C-style only, e.g., `i++`)
    pub(crate) post: Option<Box<syn::Expr>>,
    /// The loop body
    pub(crate) body: GoBlock,
}

pub(crate) enum GoForInit {
    /// Single variable with optional value: `for i := 0` or `for i := range slice`
    Single(Ident, Option<Box<syn::Expr>>),
    /// Two variables with optional value: `for i, v := range slice`
    Double(Ident, Ident, Option<Box<syn::Expr>>),
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

impl ToTokens for GoStmt {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use super::conversion::go_stmt_to_hir;
        use super::codegen::hir_stmt_to_rust;
        let hir = go_stmt_to_hir(self);
        let rust = hir_stmt_to_rust(&hir, false);
        rust.to_tokens(tokens);
    }
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
            Ok(GoForInit::Double(first, second, None))
        } else {
            Ok(GoForInit::Single(first, None))
        }
    }
}

impl Parse for GoFor {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _: syn::token::For = input.parse()?;

        // Parse optional init (either `i := 0` or nothing for C-style loops)
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
                        Some(GoForInit::Double(first_ident, second_ident, None))
                    } else {
                        let _: syn::token::Colon = input.parse()?;
                        let _: syn::token::Eq = input.parse()?;
                        Some(GoForInit::Single(first_ident, None))
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        // Detect C-style `for` vs `for` with `range`
        // C-style: `for i := 0; i < n; i++ { body }`
        // Range: `for i := 0; range slice { body }`
        // No-init C-style: `for i < n; i++ { body }`
        let is_range = if input.peek(syn::Ident) {
            let fork = input.fork();
            if let Ok(range_kw) = fork.parse::<syn::Ident>() {
                if range_kw.to_string() == "range" {
                    let _: syn::Ident = input.parse()?;
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else if input.peek(syn::token::Semi) {
            // C-style loop: no range keyword, semicolons separate init/cond/post
            false
        } else {
            true // default to range if no init and no semi
        };

        if is_range {
            // Range loop: `for init { range iterable } { body }`
            let iterable: syn::Path = input.parse()?;
            let body: GoBlock = input.parse()?;
            Ok(GoFor {
                init,
                is_range: true,
                iterable: Some(iterable),
                cond: None,
                post: None,
                body,
            })
        } else {
            // C-style loop: `for init; cond; post { body }`
            // Parse init (already done above), then `; cond; post`
            let mut cond: Option<Box<syn::Expr>> = None;
            let mut post: Option<Box<syn::Expr>> = None;

            // Optional condition (after first `;`)
            if input.peek(syn::token::Semi) {
                let _: syn::token::Semi = input.parse()?;
                // Condition can be: nothing (infinite loop), an expression, or another semi
                if !input.peek(syn::token::Semi) && !input.peek(syn::token::Brace) {
                    let expr: syn::Expr = input.parse()?;
                    cond = Some(Box::new(expr));
                }
                // If condition is empty, consume second semi
                if input.peek(syn::token::Semi) {
                    let _: syn::token::Semi = input.parse()?;
                    // Post statement
                    if !input.peek(syn::token::Brace) {
                        let expr: syn::Expr = input.parse()?;
                        post = Some(Box::new(expr));
                    }
                }
            }

            let body: GoBlock = input.parse()?;
            Ok(GoFor {
                init,
                is_range: false,
                iterable: None,
                cond,
                post,
                body,
            })
        }
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

            // Not a closure — handle as Go short declaration with raw value
            // Pattern: `name := value` (non-closure)
            // Parse the variable name up to `:=`
            let var_ident: syn::Ident = input.parse()?;
            // Consume the `:=` operator
            let _colon: syn::token::Colon = input.parse()?;
            let _equals: syn::token::Eq = input.parse()?;
            // Get the remaining value as a TokenStream
            let value_ts: TokenStream = input.parse()?;
            return Ok(GoStmt::GoShortDecl(var_ident, value_ts));
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
                    crate::debug_println!("DEBUG: parsing switch statement");
                    let result: Switch = match input.parse() {
                        Ok(s) => { crate::debug_println!("DEBUG: switch parsed ok"); s }
                        Err(e) => { crate::debug_println!("DEBUG: switch parse error: {:?}", e.to_compile_error()); return Err(e); }
                    };
                    crate::debug_println!("DEBUG: switch selector={:?}, cases={}", result.selector.is_some(), result.cases.len());
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



/// Function parameters (with shorthand grouping support).
pub(crate) struct GoFnInputs {
    pub(crate) args: Vec<GoParam>,
}

pub(crate) struct GoParam {
    pub(crate) id: Ident,
    pub(crate) ty: Option<Box<syn::Type>>,
    pub(crate) slice_elem: Option<syn::Type>,
    pub(crate) variadic: bool, // `...T` variadic parameter
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

impl ToTokens for Switch {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use super::codegen::{go_to_rust_select_hir, go_to_rust_switch_hir};
        // Use the HIR switch transpiler
        let hir_switch = super::conversion::go_switch_to_hir(self);
        let rust = super::codegen::hir_switch_to_rust_from_hir(&hir_switch);
        rust.to_tokens(tokens);
    }
}

impl ToTokens for GoSelect {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use super::codegen::go_to_rust_select_hir;
        // Use the HIR select transpiler
        let hir_select = super::conversion::go_select_to_hir(self);
        let rust = super::codegen::hir_select_to_rust_from_hir(&hir_select);
        rust.to_tokens(tokens);
    }
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

    // Pass the full closure to go_to_rust_closure_hir which handles all parsing internally
    let closure_expr = super::codegen::go_to_rust_closure_hir(tokens.clone());
    Some(closure_expr)
}

// ============================================================
// Parse impls for GoSelect and Switch — added to support
// select/switch transpilation in the HIR pipeline.
// ============================================================

impl Parse for GoSelect {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _: syn::Ident = input.parse()?; // consume 'select'
        let cases_content;
        syn::braced!(cases_content in input);
        let mut cases = Vec::new();
        while !cases_content.is_empty() {
            // Parse case keyword
            let _: syn::Ident = cases_content.parse()?; // consume 'case' or 'default'
            if cases_content.is_empty() {
                break;
            }
            // Parse case expressions or default
            eprintln!("DEBUG select: peek colon = {}", cases_content.peek(syn::token::Colon));
            if cases_content.peek(syn::token::Colon) {
                // Default case: `default: ...`
                let _: syn::token::Colon = cases_content.parse()?;
                // Empty body is valid for default
                let body: GoBlock = if !cases_content.is_empty() {
                    cases_content.parse()?
                } else {
                    GoBlock { stmts: Vec::new() }
                };
                cases.push(GoSelectCase::Default(body));
            } else {
                // Check if this is a `default` keyword (no expression before colon)
                let id_fork = cases_content.fork();
                if let Ok(id) = id_fork.parse::<syn::Ident>() {
                    let id_str = id.to_string();
                    if id_str == "default" {
                        // Consume the `default` keyword
                        let _: syn::Ident = cases_content.parse()?;
                        // Skip optional colon
                        if cases_content.peek(syn::token::Colon) {
                            let _: syn::token::Colon = cases_content.parse()?;
                        }
                        // Consume any body (block) if present
                        let body: GoBlock = if cases_content.peek(syn::token::Brace) {
                            cases_content.parse()?  
                        } else {
                            GoBlock { stmts: Vec::new() }
                        };
                        cases.push(GoSelectCase::Default(body));
                    } else {
                        // `case` keyword — parse the expression
                        let mut case_tokens = proc_macro2::TokenStream::new();
                        while !cases_content.is_empty() && !cases_content.peek(syn::token::Colon) {
                            let tt: proc_macro2::TokenTree = cases_content.parse()?;
                            case_tokens.extend(std::iter::once(tt));
                        }
                        // Consume colon
                        if cases_content.peek(syn::token::Colon) {
                            let _: syn::token::Colon = cases_content.parse()?;
                        }
                        // Parse the tokens to determine case type
                        if case_tokens.is_empty() {
                            // Empty case: `case:` (shouldn't happen, treat as default)
                            cases.push(GoSelectCase::Default(GoBlock { stmts: Vec::new() }));
                        } else {
                            let expr: Expr = syn::parse2(case_tokens)?;
                            // Check if the expression contains a `<-` send/receive
                            let src = quote! { #expr }.to_string();
                            if src.contains("< -") {
                                // Contains `<-` operator — split channel and value
                                let parts: Vec<&str> = src.split("< -").collect();
                                if parts.len() == 2 {
                                    let ch_expr: Expr = syn::parse_str(parts[0].trim())?;
                                    let val_expr: Expr = syn::parse_str(parts[1].trim())?;
                                    if cases_content.peek(syn::token::Comma) {
                                        let _: syn::token::Comma = cases_content.parse()?;
                                        // Collect value2 tokens
                                        let mut val2_tokens = proc_macro2::TokenStream::new();
                                        while !cases_content.is_empty() && !cases_content.peek(syn::token::Colon) {
                                            let tt: proc_macro2::TokenTree = cases_content.parse()?;
                                            val2_tokens.extend(std::iter::once(tt));
                                        }
                                        let val2_expr: Expr = syn::parse2(val2_tokens)?;
                                        cases.push(GoSelectCase::Send {
                                            ch: Box::new(quote! { #ch_expr }),
                                            value: Box::new(quote! { #val2_expr }),
                                        });
                                    } else {
                                        cases.push(GoSelectCase::Send {
                                            ch: Box::new(quote! { #ch_expr }),
                                            value: Box::new(quote! { #val_expr }),
                                        });
                                    }
                                } else {
                                    // Recv: `<-ch` — first part before `<-` is empty or contains target
                                    cases.push(GoSelectCase::Recv {
                                        ch: Box::new(quote! { #expr }),
                                        target: None,
                                    });
                                }
                            } else {
                                // Simple expression case
                                cases.push(GoSelectCase::Send {
                                    ch: Box::new(quote! { #expr }),
                                    value: Box::new(quote! { () }),
                                });
                            }
                        }
                    }
                } else {
                    // Not an ident — try to parse as expression (edge case)
                    let expr: Expr = cases_content.parse()?;
                    cases.push(GoSelectCase::Send {
                        ch: Box::new(quote! { #expr }),
                        value: Box::new(quote! { () }),
                    });
                }
            }
            // After any non-default case, consume the colon
            if cases_content.peek(syn::token::Colon) {
                let _: syn::token::Colon = cases_content.parse()?;
            }
        }
        Ok(GoSelect { cases })
    }
}

impl Parse for Switch {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _: syn::Ident = input.call(Ident::parse_any)?;

        // Parse optional selector expression (stop at `{` boundary)
        let selector = if input.peek(syn::token::Brace) {
            None
        } else {
            let path: syn::Path = input.parse()?;
            Some(syn::Expr::Path(syn::ExprPath {
                attrs: Vec::new(),
                qself: None,
                path,
            }))
        };

        let cases_content;
        syn::braced!(cases_content in input);

        let mut cases = Vec::new();
        let mut default_stmts = Vec::new();

        while !cases_content.is_empty() {
            let fork = cases_content.fork();
            if fork.peek(syn::Ident) {
                if let Ok(kw) = fork.parse::<syn::Ident>() {
                    let kw_str = kw.to_string();
                    if kw_str == "case" {
                        cases_content.parse::<syn::Ident>()?;

                        let mut exprs = Vec::new();
                        while !cases_content.peek(syn::token::Colon) && !cases_content.is_empty() {
                            let kw_fork = cases_content.fork();
                            if kw_fork.peek(syn::Ident) {
                                let kw = kw_fork.parse::<syn::Ident>();
                                if let Ok(kw) = kw {
                                    let kw_str = kw.to_string();
                                    if matches!(kw_str.as_str(),
                                        "if" | "for" | "return" | "switch" | "case" | "default") {
                                        break;
                                    }
                                }
                            }
                            let case_fork = cases_content.fork();
                            if case_fork.peek(syn::Lit) {
                                let lit: syn::Lit = case_fork.parse()?;
                                cases_content.advance_to(&case_fork);
                                exprs.push(Expr::Lit(syn::ExprLit {
                                    attrs: Vec::new(),
                                    lit,
                                }));
                            } else if case_fork.peek(syn::Ident) {
                                let path: syn::Path = case_fork.parse()?;
                                cases_content.advance_to(&case_fork);
                                exprs.push(Expr::Path(syn::ExprPath {
                                    attrs: Vec::new(),
                                    qself: None,
                                    path,
                                }));
                            } else {
                                if cases_content.peek(syn::token::Comma) {
                                    let _: syn::token::Comma = cases_content.parse()?;
                                } else {
                                    cases_content.parse::<proc_macro2::TokenTree>()?;
                                }
                            }
                        }
                        let _: syn::token::Colon = cases_content.parse()?;

                        // Parse case body: individual statements, not all-remaining tokens
                        let mut body_stmts = Vec::new();
                        while !cases_content.is_empty() && !cases_content.peek(syn::Ident) {
                            let stmt_fork = cases_content.fork();
                            if let Ok(expr) = stmt_fork.parse::<Expr>() {
                                cases_content.advance_to(&stmt_fork);
                                body_stmts.push(GoStmt::Expr(expr));
                            } else {
                                break;
                            }
                        }

                        cases.push(SwitchCase { exprs, stmts: body_stmts });
                        continue;
                    } else if kw_str == "default" {
                        cases_content.parse::<syn::Ident>()?;
                        let _: syn::token::Colon = cases_content.parse()?;

                        // Parse default body: individual statements
                        let mut body_stmts = Vec::new();
                        while !cases_content.is_empty() && !cases_content.peek(syn::Ident) {
                            let stmt_fork = cases_content.fork();
                            if let Ok(expr) = stmt_fork.parse::<Expr>() {
                                cases_content.advance_to(&stmt_fork);
                                body_stmts.push(GoStmt::Expr(expr));
                            } else {
                                break;
                            }
                        }

                        default_stmts = body_stmts;
                        continue;
                    }
                }
            }
            break;
        }

        Ok(Switch { selector, cases, default_stmts })
    }
}

use syn::parse::discouraged::Speculative as _;

impl Parse for GoFn {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _fn_kw: Ident = input.call(Ident::parse_any)?;
        let ident: Ident = input.parse()?;
        let generics = Punctuated::<syn::GenericParam, token::Comma>::new();
        if input.peek(syn::token::Bracket) {
            let content;
            let _bracketed = syn::bracketed!(content in input);
            Punctuated::<syn::GenericParam, token::Comma>::parse_terminated(&content)?;
        }
        let paren_content;
        let _paren = syn::parenthesized!(paren_content in input);
        let inputs: GoFnInputs = paren_content.parse()?;

        // Parse return type or block
        let output = if input.peek(syn::token::Brace) {
            // No return type — cursor is directly at the body
            None
        } else if input.peek(syn::Ident) {
            let fork = input.fork();
            if let Ok(first_ident) = fork.parse::<syn::Ident>() {
                let first_name = first_ident.to_string();
                if first_name == "chan" || first_name == "map" {
                    // Complex type — use GoFnOutput::parse
                    Some(input.parse()?)
                } else {
                    // Simple type like `int`, `string` — parse directly.
                    let consumed_ident: syn::Ident = input.parse()?;
                    // Check for generic type like `Vec<i32>` — use fork + advance_to
                    let path = if input.peek(syn::token::Lt) {
                        let remaining = input.fork();
                        let args: syn::AngleBracketedGenericArguments =
                            remaining.parse()?;
                        let mut p = syn::Path::from(consumed_ident);
                        p.segments.last_mut().unwrap().arguments =
                            syn::PathArguments::AngleBracketed(args);
                        input.advance_to(&remaining);
                        p
                    } else {
                        syn::Path::from(consumed_ident)
                    };
                    let t: syn::Type = syn::Type::Path(syn::TypePath {
                        path,
                        qself: None,
                    });
                    Some(GoFnOutput {
                        tys: vec![t],
                        is_slice: false,
                        elem_type: None,
                    })
                }
            } else {
                Some(input.parse()?)
            }
        } else if !input.is_empty() {
            Some(input.parse()?)
        } else {
            None
        };

        let block: GoBlock = super::statement::parse_go_block(input)?;
        Ok(GoFn { ident, generics, inputs, output, block })
    }
}

impl Parse for GoStruct {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _struct_kw: Ident = input.call(Ident::parse_any)?;
        let ident: Ident = input.parse()?;

        // Parse fields
        let content;
        let _braced = syn::braced!(content in input);
        let mut fields = Vec::new();
        while !content.is_empty() {
            let name: Ident = content.parse()?;
            // Skip optional comma between fields
            if content.peek(Token![,]) {
                let _: Token![,] = content.parse()?;
            }
            let ty: syn::Type = content.parse()?;
            fields.push(GoStructField { name, ty });
        }

        Ok(GoStruct { ident, fields })
    }
}

// Parse implementations for GoFnInputs and GoFnOutput
// Copied from legacy params.rs, adapted to HIR types.

impl Parse for GoFnInputs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        use super::types::map_go_type_str;
        let mut args = Vec::new();
        while !input.is_empty() {
            let id: Ident = input.parse()?;
            let mut group_ids: Vec<Ident> = Vec::new();

            // Look ahead for grouped parameters: `a, b, c int`
            let fork = input.fork();
            // Adjust fork for variadic: skip `...` if present
            if fork.peek(syn::token::DotDotDot) {
                let _ = fork.parse::<syn::token::DotDotDot>();
            }
            let mut ty_from_ident: Option<Box<syn::Type>> = None;

            // Detect variadic parameter: `name ...T`
            let is_variadic = input.peek(syn::token::DotDotDot);
            if is_variadic {
                let _: syn::token::DotDotDot = input.parse()?;
            }

            // Detect grouped params: after parsing the first param (`a`), peek ahead
            while input.peek(token::Comma) {
                let peek_fork = input.fork();
                let _ = peek_fork.parse::<token::Comma>();
                if peek_fork.peek(Ident) {
                    let name = peek_fork.parse::<Ident>()?;
                    let name_str = name.to_string();
                    let known_go_type = matches!(name_str.as_str(),
                        "bool" | "string" | "int" | "int8" | "int16" | "int32" | "int64"
                        | "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr"
                        | "byte" | "rune" | "float32" | "float64" | "error" | "chan"
                    );
                    if known_go_type {
                        input.advance_to(&peek_fork);
                        ty_from_ident = Some(Box::new(input.parse()?));
                        break;
                    }
                    input.parse::<token::Comma>()?;
                    let param_name: Ident = input.parse()?;
                    group_ids.push(param_name);
                } else {
                    break;
                }
            }

            let fork = input.fork();
            let is_slice_like = fork.peek(syn::token::Bracket);
            let is_map_like = {
                let f = fork.fork();
                f.parse::<Ident>().ok().map(|id| id.to_string() == "map").unwrap_or(false)
            };

            if ty_from_ident.is_none() {
                if !is_slice_like && !is_map_like && fork.peek(syn::Ident) {
                    ty_from_ident = Some(input.parse()?);
                } else if !is_slice_like && fork.peek(syn::token::Colon) {
                    let _colon: syn::token::Colon = input.parse()?;
                    ty_from_ident = Some(input.parse()?);
                }
            }

            if is_slice_like {
                let content;
                let _ = syn::bracketed!(content in input);
                let elem_path: syn::Path = if content.is_empty() {
                    input.parse()?
                } else {
                    content.parse()?
                };
                let elem_type = syn::Type::Path(syn::TypePath {
                    path: elem_path,
                    qself: None,
                });
                args.push(GoParam { id: id.clone(), ty: None, slice_elem: Some(elem_type.clone()), variadic: is_variadic });
                for param_id in group_ids {
                    args.push(GoParam { id: param_id, ty: None, slice_elem: Some(elem_type.clone()), variadic: is_variadic });
                }
            } else if is_map_like {
                let _: Ident = input.parse()?; // consume `map`
                let k_content;
                let _bracket = syn::bracketed!(k_content in input);
                let key_type: syn::Type = k_content.parse().unwrap_or_else(|_| {
                    syn::Type::Path(syn::TypePath {
                        path: syn::Path::from(syn::Ident::new("string", proc_macro2::Span::call_site())),
                        qself: None,
                    })
                });
                let val_type: syn::Type = if input.peek(syn::Ident) {
                    input.parse().unwrap_or_else(|_| {
                        syn::Type::Path(syn::TypePath {
                            path: syn::Path::from(syn::Ident::new("int", proc_macro2::Span::call_site())),
                            qself: None,
                        })
                    })
                } else {
                    syn::Type::Path(syn::TypePath {
                        path: syn::Path::from(syn::Ident::new("int", proc_macro2::Span::call_site())),
                        qself: None,
                    })
                };
                let mut map_path = syn::Path::from(syn::Ident::new("__go_map", proc_macro2::Span::call_site()));
                map_path.segments.clear();
                map_path.segments.push(syn::PathSegment {
                    ident: syn::Ident::new("__go_map", proc_macro2::Span::call_site()),
                    arguments: syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                        colon2_token: None,
                        lt_token: Token![<](proc_macro2::Span::call_site()),
                        args: syn::punctuated::Punctuated::from_iter([
                            syn::GenericArgument::Type(key_type),
                            syn::GenericArgument::Type(val_type),
                        ]),
                        gt_token: Token![>](proc_macro2::Span::call_site()),
                    }),
                });
                let map_type: Box<syn::Type> = Box::new(syn::Type::Path(syn::TypePath { path: map_path, qself: None }));
                let map_ty_clone = map_type.clone();
                args.push(GoParam { id: id.clone(), ty: Some(map_type), slice_elem: None, variadic: is_variadic });
                for param_id in group_ids {
                    args.push(GoParam { id: param_id, ty: Some(map_ty_clone.clone()), slice_elem: None, variadic: is_variadic });
                }
            } else {
                let ty = if let Some(ty) = ty_from_ident.clone() {
                    if let syn::Type::Path(tp) = &*ty
                        && tp.path.segments.len() == 1
                        && tp.path.segments.first().unwrap().ident.to_string() == "chan"
                    {
                        if input.peek(syn::Ident) {
                            let elem_ty: syn::Type = input.parse()?;
                            let mut chan_path = syn::Path::from(syn::Ident::new("chan", proc_macro2::Span::call_site()));
                            chan_path.segments.clear();
                            chan_path.segments.push(syn::PathSegment {
                                ident: syn::Ident::new("chan", proc_macro2::Span::call_site()),
                                arguments: syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                                    colon2_token: Default::default(),
                                    lt_token: Token![<](proc_macro2::Span::call_site()),
                                    args: syn::punctuated::Punctuated::from_iter([syn::GenericArgument::Type(elem_ty)]),
                                    gt_token: Token![>](proc_macro2::Span::call_site()),
                                }),
                            });
                            Some(Box::new(syn::Type::Path(syn::TypePath { path: chan_path, qself: None })))
                        } else {
                            Some(ty)
                        }
                    } else {
                        Some(ty)
                    }
                } else { None };
                let ty_for_param = ty.clone();
                args.push(GoParam { id: id.clone(), ty: ty_for_param, slice_elem: None, variadic: is_variadic });
                for param_id in group_ids {
                    args.push(GoParam { id: param_id, ty: ty.clone(), slice_elem: None, variadic: is_variadic });
                }
            }

            if input.peek(token::Comma) {
                input.parse::<token::Comma>()?;
            }
        }
        Ok(GoFnInputs { args })
    }
}

impl Parse for GoFnOutput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        use super::types::map_go_type_str;
        let mut tys: Vec<syn::Type> = Vec::new();
        let mut is_slice = false;
        let mut elem_type: Option<Box<syn::Type>> = None;
        if input.peek(syn::token::RArrow) {
            let _: syn::token::RArrow = input.parse()?;
        }
        if !input.peek(syn::token::Brace) {
            // Handle parenthesized multi-return: `(int, int)` → `(String, i32)`
            if input.peek(syn::token::Paren) {
                let content;
                let _paren = syn::parenthesized!(content in input);
                while !content.is_empty() {
                    let ty: syn::Type = content.parse()?;
                    tys.push(ty);
                    if content.peek(Token![,]) {
                        let _: Token![,] = content.parse()?;
                    }
                }
            } else if input.peek(syn::token::Bracket) {
                is_slice = true;
                let content;
                let _bracket = syn::bracketed!(content in input);
                if input.peek(syn::token::Brace) {
                    tys.push(syn::Type::Path(syn::TypePath {
                        path: syn::Path::from(syn::Ident::new("__go_slice__", proc_macro2::Span::call_site())),
                        qself: None,
                    }));
                } else {
                    let elem = input.parse::<syn::Type>()?;
                    elem_type = Some(Box::new(elem));
                    tys.push(syn::Type::Path(syn::TypePath {
                        path: syn::Path::from(syn::Ident::new("__go_slice__", proc_macro2::Span::call_site())),
                        qself: None,
                    }));
                }
            } else if input.peek(syn::Ident) {
                let fork = input.fork();
                if let Ok(first_ident) = fork.parse::<syn::Ident>() {
                    let first_name = first_ident.to_string();
                    if first_name == "chan" {
                        let _: syn::Ident = input.parse()?;
                        let elem = if input.peek(syn::token::Bracket) {
                            let content;
                            let _bracket = syn::bracketed!(content in input);
                            content.parse::<syn::Type>().unwrap_or_else(|_| {
                                syn::Type::Path(syn::TypePath {
                                    path: syn::Path::from(syn::Ident::new("i32", proc_macro2::Span::call_site())),
                                    qself: None,
                                })
                            })
                        } else if input.peek(syn::Ident) {
                            input.parse::<syn::Type>().unwrap_or_else(|_| {
                                syn::Type::Path(syn::TypePath {
                                    path: syn::Path::from(syn::Ident::new("i32", proc_macro2::Span::call_site())),
                                    qself: None,
                                })
                            })
                        } else {
                            syn::Type::Path(syn::TypePath {
                                path: syn::Path::from(syn::Ident::new("i32", proc_macro2::Span::call_site())),
                                qself: None,
                            })
                        };
                        let mut chan_path = syn::Path::from(syn::Ident::new("__go_chan", proc_macro2::Span::call_site()));
                        chan_path.segments.clear();
                        chan_path.segments.push(syn::PathSegment {
                            ident: syn::Ident::new("__go_chan", proc_macro2::Span::call_site()),
                            arguments: syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                                colon2_token: None,
                                lt_token: Token![<](proc_macro2::Span::call_site()),
                                args: syn::punctuated::Punctuated::from_iter([
                                    syn::GenericArgument::Type(elem)
                                ]),
                                gt_token: Token![>](proc_macro2::Span::call_site()),
                            }),
                        });
                        tys.push(syn::Type::Path(syn::TypePath {
                            path: chan_path,
                            qself: None,
                        }));
                    } else if first_name == "map" {
                        let _: syn::Ident = input.parse()?;
                        if input.peek(syn::token::Bracket) {
                            let k_content;
                            let _bracket = syn::bracketed!(k_content in input);
                            let key_type: syn::Type = k_content.parse().unwrap_or_else(|_| {
                                syn::Type::Path(syn::TypePath {
                                    path: syn::Path::from(syn::Ident::new("string", proc_macro2::Span::call_site())),
                                    qself: None,
                                })
                            });
                            let val_type: syn::Type = if input.peek(syn::Ident) {
                                input.parse().unwrap_or_else(|_| {
                                    syn::Type::Path(syn::TypePath {
                                        path: syn::Path::from(syn::Ident::new("int", proc_macro2::Span::call_site())),
                                        qself: None,
                                    })
                                })
                            } else {
                                syn::Type::Path(syn::TypePath {
                                    path: syn::Path::from(syn::Ident::new("int", proc_macro2::Span::call_site())),
                                    qself: None,
                                })
                            };
                            let mut map_path = syn::Path::from(syn::Ident::new("__go_map", proc_macro2::Span::call_site()));
                            map_path.segments.clear();
                            map_path.segments.push(syn::PathSegment {
                                ident: syn::Ident::new("__go_map", proc_macro2::Span::call_site()),
                                arguments: syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                                    colon2_token: None,
                                    lt_token: Token![<](proc_macro2::Span::call_site()),
                                    args: syn::punctuated::Punctuated::from_iter([
                                        syn::GenericArgument::Type(key_type),
                                        syn::GenericArgument::Type(val_type),
                                    ]),
                                    gt_token: Token![>](proc_macro2::Span::call_site()),
                                }),
                            });
                            tys.push(syn::Type::Path(syn::TypePath {
                                path: map_path,
                                qself: None,
                            }));
                        }
                    } else {
                        let t = input.parse::<syn::Type>()?;
                        tys.push(t);
                    }
                } else {
                    let t = input.parse::<syn::Type>()?;
                    tys.push(t);
                }
            } else {
                let t = input.parse()?;
                tys.push(t);
            }
            while input.peek(token::Comma) {
                let _ = input.parse::<token::Comma>()?;
                if input.peek(syn::token::Brace) {
                    break;
                }
                if input.peek(syn::token::Bracket) {
                    is_slice = true;
                    let content;
                    let _bracket = syn::bracketed!(content in input);
                    if input.peek(syn::token::Brace) {
                        tys.push(syn::Type::Path(syn::TypePath {
                            path: syn::Path::from(syn::Ident::new("__go_slice__", proc_macro2::Span::call_site())),
                            qself: None,
                        }));
                    } else {
                        let elem = input.parse::<syn::Type>()?;
                        elem_type = Some(Box::new(elem));
                        tys.push(syn::Type::Path(syn::TypePath {
                            path: syn::Path::from(syn::Ident::new("__go_slice__", proc_macro2::Span::call_site())),
                            qself: None,
                        }));
                    }
                } else {
                    let t = input.parse()?;
                    tys.push(t);
                }
            }
        }
        Ok(GoFnOutput { tys, is_slice, elem_type })
    }
}

// Parse implementations for interface types.
// These are needed by free_fn/interface.rs which calls syn::parse2::<GoInterface>(input).

impl Parse for GoInterfaceMethod {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.call(Ident::parse_any)?;
        let inputs: GoFnInputs = input.parse()?;
        let output: Option<GoFnOutput> = if input.peek(syn::token::Brace) {
            None
        } else {
            Some(input.parse()?)
        };
        Ok(GoInterfaceMethod { name, inputs, output })
    }
}

impl Parse for GoInterface {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let _interface_kw: Ident = input.call(Ident::parse_any)?;
        let ident: Ident = input.parse()?;

        // Parse methods inside braces
        let content;
        let _braced = syn::braced!(content in input);
        let mut methods = Vec::new();
        while !content.is_empty() {
            let method_fork = content.fork();
            if let Ok(method) = method_fork.parse::<GoInterfaceMethod>() {
                content.advance_to(&method_fork);
                methods.push(method);
            } else {
                break;
            }
        }

        Ok(GoInterface { ident, methods })
    }
}

impl Parse for GoParam {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let id: Ident = input.parse()?;
        let _colon: Token![:] = input.parse()?;

        // Check for variadic `...T`
        let variadic = if input.peek(Token![...]) {
            let _dot3: Token![...] = input.parse()?;
            true
        } else {
            false
        };

        // Parse the type
        let ty: Option<Box<syn::Type>> = if input.peek(syn::token::Bracket) {
            // Slice type like `[]T` — special marker
            let content;
            let _bracket = syn::bracketed!(content in input);
            let elem = if content.is_empty() {
                syn::Type::Path(syn::TypePath { path: syn::Path::from(Ident::new("i32", proc_macro2::Span::call_site())), qself: None })
            } else {
                content.parse::<syn::Type>()?
            };
            None
        } else if input.peek(syn::Ident) {
            Some(Box::new(input.parse::<syn::Type>()?))
        } else {
            None
        };

        Ok(GoParam { id, ty, slice_elem: None, variadic })
    }
}
