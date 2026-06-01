//! Semantic validation for Go and Rust code.
//!
//! These functions write code to temp directories and invoke the real compilers
//! to check whether the code would actually compile. Designed to run at
//! **compile time** of the user's crate (inside proc macros), so the user
//! gets immediate, accurate error messages.

use proc_macro2::TokenStream;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Validate that `go_code` would compile as valid Go.
///
/// Writes the code to a temporary Go module directory and runs `go build`.
/// Returns `Err(msg)` with the compiler output if it fails.
///
/// The input should be Go **declarations** (structs, functions, etc.).
/// A minimal `package main` + `main()` harness is added automatically.
pub fn validate_go(go_code: &TokenStream) -> Result<(), String> {
    let dir = temp_dir("gourd-go");
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create temp dir: {}", e))?;
    fs::write(dir.join("go.mod"), "module gourd-test\ngo 1.21\n")
        .map_err(|e| format!("failed to write go.mod: {}", e))?;
    let harness = go_to_main_harness(go_code);
    eprintln!("GO HARNESS: [{}]", harness);
    fs::write(dir.join("main.go"), harness)
        .map_err(|e| format!("failed to write main.go: {}", e))?;

    let output = Command::new("go")
        .args(["build", "-o", "/dev/null", "."])
        .current_dir(&dir)
        .output()
        .map_err(|e| format!("failed to run `go build`: {}", e))?;

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        fs::remove_dir_all(&dir).ok();
        return Err(stderr.trim().to_string());
    }

    fs::remove_dir_all(&dir).ok();
    Ok(())
}

/// Validate that `rust_code` would compile as valid Rust.
///
/// Writes the code to a temporary Rust project directory and runs
/// `cargo check`. Returns `Err(msg)` with the compiler output if it fails.
///
/// The input should be Rust **declarations** (structs, functions, impls, etc.).
/// A minimal main function is added automatically.
pub fn validate_rust(rust_code: &TokenStream) -> Result<(), String> {
    let dir = temp_dir("gourd-rust");
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create temp dir: {}", e))?;
    fs::write(dir.join("Cargo.toml"),
        "[package]\nname = \"gourd-test\"\nversion = \"0.0.0\"\nedition = \"2021\"\n")
        .map_err(|e| format!("failed to write Cargo.toml: {}", e))?;
    let src = dir.join("src");
    fs::create_dir_all(&src).map_err(|e| format!("failed to create src/: {}", e))?;
    fs::write(src.join("main.rs"), rust_to_main_harness(rust_code))
        .map_err(|e| format!("failed to write src/main.rs: {}", e))?;

    let output = Command::new("cargo")
        .args(["check", "-q"])
        .current_dir(&dir)
        .output()
        .map_err(|e| format!("failed to run `cargo check`: {}", e))?;

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        fs::remove_dir_all(&dir).ok();
        return Err(stderr.trim().to_string());
    }

    fs::remove_dir_all(&dir).ok();
    Ok(())
}

/// Create a unique temporary directory for this run.
fn temp_dir(prefix: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{}_{}_{:x}", prefix, std::process::id(), n))
}

/// Convert token stream to string, preserving original formatting.
/// Adds semicolons between statements when needed for Go validation.
fn ts_to_string(ts: &TokenStream) -> String {
    use proc_macro2::TokenTree;
    let mut s = String::new();
    let mut need_space = false;
    let mut expect_statement_end = false;

    for tree in ts.clone().into_iter() {
        match tree {
            TokenTree::Ident(id) => {
                let id_str = id.to_string();
                // Insert semicolon before statement keywords when transitioning
                // from a statement-ending context to a new statement
                if expect_statement_end
                    && matches!(id_str.as_str(), "if" | "for" | "switch" | "return" | "break" | "continue")
                {
                    s.push(';');
                }
                if need_space && !s.is_empty() && !s.ends_with(' ') {
                    s.push(' ');
                }
                s.push_str(&id_str);
                need_space = true;
                // These keywords end a statement context
                expect_statement_end = !matches!(id_str.as_str(),
                    "else" | "case" | "default" | "map" | "struct" | "func");
            }
            TokenTree::Literal(lit) => {
                if need_space && !s.is_empty() && !s.ends_with(' ') {
                    s.push(' ');
                }
                s.push_str(&lit.to_string());
                need_space = false;
                expect_statement_end = true;
            }
            TokenTree::Group(g) => {
                if need_space && !s.is_empty() && !s.ends_with(' ') {
                    s.push(' ');
                }
                let inner = ts_to_string(&g.stream());
                match g.delimiter() {
                    proc_macro2::Delimiter::Parenthesis => s.push_str(&format!("({})", inner)),
                    proc_macro2::Delimiter::Brace => s.push_str(&format!("{{{}}}", inner)),
                    proc_macro2::Delimiter::Bracket => s.push_str(&format!("[{}]", inner)),
                    proc_macro2::Delimiter::None => s.push_str(&inner),
                }
                need_space = true;
                // A closing brace often ends a statement
                expect_statement_end = matches!(g.delimiter(),
                    proc_macro2::Delimiter::Brace | proc_macro2::Delimiter::Bracket | proc_macro2::Delimiter::Parenthesis);
            }
            TokenTree::Punct(p) => {
                let ch = p.as_char();
                s.push(ch);
                need_space = true;
                // Semicolons are explicit statement terminators
                if ch == ';' {
                    expect_statement_end = false;
                }
            }
        }
    }
    s
}

/// Wrap Go declarations in a minimal `package main` harness.
fn go_to_main_harness(go_code: &TokenStream) -> String {
    let code = ts_to_string(go_code);

    // `quote!` adds spaces around punctuation (e.g., `func hello ( ) string`).
    // Fix: remove space before `(`, `)`, `{`, `}`, `[`, `]`.
    // `func hello ( ) string` → `func hello() string`
    let code = fix_quote_spaces(&code);

    // Build the harness: wrap declarations in package main with a main() function.
    // Go rejects `package main` without imports or a main() function.
    let mut harness = String::new();
    harness.push_str("package main\n\n");
    harness.push_str(&code);
    harness.push_str("\n\nfunc main() {\n}");
    harness
}

fn fix_quote_spaces(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut prev: Option<char> = None;

    while let Some(c) = chars.next() {
        // Remove space before closing delimiters: ` ) ` → `)`
        if (c == ')' || c == '}' || c == ']') && prev == Some(' ') {
            result.pop();
        }
        result.push(c);
        prev = Some(c);
    }
    result
}

/// Wrap Rust declarations in a minimal `fn main()` harness.
fn rust_to_main_harness(rust_code: &TokenStream) -> String {
    let code = ts_to_string(rust_code);

    let mut harness = String::new();
    harness.push_str("fn main() {\n");
    harness.push_str("}\n");
    harness.push_str(&code);

    harness
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::Literal;

    #[test]
    fn test_valid_go() {
        let code: TokenStream = quote::quote! {
            func hello() string {
                return "world"
            }
        };
        assert!(validate_go(&code).is_ok());
    }

    #[test]
    fn test_invalid_go() {
        let code: TokenStream = quote::quote! {
            func hello() string {
                return bad_expression!!
            }
        };
        let err = validate_go(&code).unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn test_valid_rust() {
        let code: TokenStream = quote::quote! {
            fn hello() -> String {
                String::from("world")
            }
        };
        match validate_rust(&code) {
            Ok(_) => {}
            Err(e) => eprintln!("RUST VALID ERR: {e}"),
        }
        assert!(validate_rust(&code).is_ok());
    }

    #[test]
    fn test_invalid_rust() {
        let code: TokenStream = quote::quote! {
            fn hello() -> String {
                return bad_expression!!
            }
        };
        let err = validate_rust(&code).unwrap_err();
        assert!(!err.is_empty());
    }
}
