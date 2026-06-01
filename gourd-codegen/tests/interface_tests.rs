//! Interface transpilation tests.
//!
//! Tests that Go interface declarations are transpiled to Rust trait definitions.

use gourd_codegen_core::transpile_go_text;

// ── Basic interface: single method ─────────────────────────────────

#[test]
fn test_basic_interface() {
    let code = "interface Shape { Name() string }";
    let rust = transpile_go_text(code);
    let rust_str = quote::quote! { #rust }.to_string();
    assert!(rust_str.contains("trait shape"), "expected 'trait shape' in: {}", rust_str);
    assert!(rust_str.contains("fn name"), "expected 'fn name' in: {}", rust_str);
    assert!(rust_str.contains("-> String"), "expected '-> String' in: {}", rust_str);
}

// ── Interface with multiple methods ──────────────────────────────────

#[test]
fn test_multi_method_interface() {
    let code = "interface Point { X() int Y() int }";
    let rust = transpile_go_text(code);
    let rust_str = quote::quote! { #rust }.to_string();
    assert!(rust_str.contains("trait point"), "expected 'trait point' in: {}", rust_str);
    assert!(rust_str.contains("fn x"), "expected 'fn x' in: {}", rust_str);
    assert!(rust_str.contains("fn y"), "expected 'fn y' in: {}", rust_str);
    assert!(rust_str.contains("-> i32"), "expected '-> i32' in: {}", rust_str);
}

// ── Interface with parameters ────────────────────────────────────────

#[test]
fn test_interface_with_params() {
    let code = "interface Printer { Print(msg string) }";
    let rust = transpile_go_text(code);
    let rust_str = quote::quote! { #rust }.to_string();
    assert!(rust_str.contains("trait printer"), "expected 'trait printer' in: {}", rust_str);
    assert!(rust_str.contains("fn print"), "expected 'fn print' in: {}", rust_str);
    assert!(rust_str.contains("msg : String"), "expected 'msg : String' in: {}", rust_str);
}

// ── Interface with mixed return types ────────────────────────────────

#[test]
fn test_interface_mixed_returns() {
    let code = "interface Data { Id() int Name() string }";
    let rust = transpile_go_text(code);
    let rust_str = quote::quote! { #rust }.to_string();
    assert!(rust_str.contains("trait data"), "expected 'trait data' in: {}", rust_str);
    assert!(rust_str.contains("fn id"), "expected 'fn id' in: {}", rust_str);
    assert!(rust_str.contains("fn name"), "expected 'fn name' in: {}", rust_str);
    assert!(rust_str.contains("-> i32"), "expected '-> i32' in: {}", rust_str);
    assert!(rust_str.contains("-> String"), "expected '-> String' in: {}", rust_str);
}

// ── Interface with slice method ──────────────────────────────────────

#[test]
fn test_interface_slice_method() {
    let code = "interface Reader { Read(data []byte) []byte }";
    let rust = transpile_go_text(code);
    let rust_str = quote::quote! { #rust }.to_string();
    assert!(rust_str.contains("trait reader"), "expected 'trait reader' in: {}", rust_str);
    assert!(rust_str.contains("fn read"), "expected 'fn read' in: {}", rust_str);
    assert!(rust_str.contains("data : &"), "expected 'data : &' in: {}", rust_str);
    assert!(rust_str.contains("[u8]"), "expected '[u8]' in: {}", rust_str);
    assert!(rust_str.contains("-> Vec"), "expected '-> Vec' in: {}", rust_str);
}

// ── Interface with parameter grouping ────────────────────────────────

#[test]
fn test_interface_with_param_grouping() {
    let code = "interface Math { Add(a, b int) int }";
    let rust = transpile_go_text(code);
    let rust_str = quote::quote! { #rust }.to_string();
    assert!(rust_str.contains("trait math"), "expected 'trait math' in: {}", rust_str);
    assert!(rust_str.contains("fn add"), "expected 'fn add' in: {}", rust_str);
    assert!(rust_str.contains("a : i32"), "expected 'a : i32' in: {}", rust_str);
    assert!(rust_str.contains("b : i32"), "expected 'b : i32' in: {}", rust_str);
    assert!(rust_str.contains("-> i32"), "expected '-> i32' in: {}", rust_str);
}

// ── Empty interface (bare `interface{}`) ────────────────────────────

#[test]
fn test_empty_interface_name() {
    let code = "interface Empty {}";
    let rust = transpile_go_text(code);
    let rust_str = quote::quote! { #rust }.to_string();
    assert!(rust_str.contains("trait empty"), "expected 'trait empty' in: {}", rust_str);
}
