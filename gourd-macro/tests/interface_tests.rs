//! Interface transpilation tests.
//!
//! Tests that Go interface declarations are transpiled to Rust trait definitions.


// ── Basic interface: single method ─────────────────────────────────

#[test]
fn test_basic_interface() {
    let code = "interface Shape { Name() string }";
    let rust = gourd_codegen::transpile_go_text(code);
    let rust_str = quote::quote! { #rust }.to_string();
    assert!(rust_str.contains("trait Shape"), "expected 'trait Shape' in: {}", rust_str);
    assert!(rust_str.contains("fn name"), "expected 'fn name' in: {}", rust_str);
    assert!(rust_str.contains("-> String"), "expected '-> String' in: {}", rust_str);
}

// ── Interface with multiple methods ──────────────────────────────────

#[test]
fn test_multi_method_interface() {
    let code = "interface Point { X() int Y() int }";
    let rust = gourd_codegen::transpile_go_text(code);
    let rust_str = quote::quote! { #rust }.to_string();
    assert!(rust_str.contains("trait Point"), "expected 'trait Point' in: {}", rust_str);
    assert!(rust_str.contains("fn x"), "expected 'fn x' in: {}", rust_str);
    assert!(rust_str.contains("fn y"), "expected 'fn y' in: {}", rust_str);
    assert!(rust_str.contains("-> i32"), "expected '-> i32' in: {}", rust_str);
}

// ── Interface with parameters ────────────────────────────────────────

#[test]
fn test_interface_with_params() {
    let code = "interface Printer { Print(msg string) }";
    let rust = gourd_codegen::transpile_go_text(code);
    let rust_str = quote::quote! { #rust }.to_string();
    assert!(rust_str.contains("trait Printer"), "expected 'trait Printer' in: {}", rust_str);
    assert!(rust_str.contains("fn print"), "expected 'fn print' in: {}", rust_str);
    assert!(rust_str.contains("msg : String"), "expected 'msg : String' in: {}", rust_str);
}

// ── Interface with mixed return types ────────────────────────────────

#[test]
fn test_interface_mixed_returns() {
    let code = "interface Data { Id() int Name() string }";
    let rust = gourd_codegen::transpile_go_text(code);
    let rust_str = quote::quote! { #rust }.to_string();
    assert!(rust_str.contains("trait Data"), "expected 'trait Data' in: {}", rust_str);
    assert!(rust_str.contains("fn id"), "expected 'fn id' in: {}", rust_str);
    assert!(rust_str.contains("fn name"), "expected 'fn name' in: {}", rust_str);
    assert!(rust_str.contains("-> i32"), "expected '-> i32' in: {}", rust_str);
    assert!(rust_str.contains("-> String"), "expected '-> String' in: {}", rust_str);
}

// ── Interface with slice method ──────────────────────────────────────

#[test]
fn test_interface_slice_method() {
    let code = "interface Reader { Read(data []byte) []byte }";
    let rust = gourd_codegen::transpile_go_text(code);
    let rust_str = quote::quote! { #rust }.to_string();
    assert!(rust_str.contains("trait Reader"), "expected 'trait Reader' in: {}", rust_str);
    assert!(rust_str.contains("fn read"), "expected 'fn read' in: {}", rust_str);
    assert!(rust_str.contains("data : &"), "expected 'data : &' in: {}", rust_str);
    assert!(rust_str.contains("[u8]"), "expected '[u8]' in: {}", rust_str);
    assert!(rust_str.contains("-> Vec"), "expected '-> Vec' in: {}", rust_str);
}

// ── Interface with parameter grouping ────────────────────────────────

#[test]
fn test_interface_with_param_grouping() {
    let code = "interface Math { Add(a, b int) int }";
    let rust = gourd_codegen::transpile_go_text(code);
    let rust_str = quote::quote! { #rust }.to_string();
    assert!(rust_str.contains("trait Math"), "expected 'trait Math' in: {}", rust_str);
    assert!(rust_str.contains("fn add"), "expected 'fn add' in: {}", rust_str);
    assert!(rust_str.contains("a : i32"), "expected 'a : i32' in: {}", rust_str);
    assert!(rust_str.contains("b : i32"), "expected 'b : i32' in: {}", rust_str);
    assert!(rust_str.contains("-> i32"), "expected '-> i32' in: {}", rust_str);
}

// ── Empty interface (bare `interface{}`) ────────────────────────────

#[test]
fn test_empty_interface_name() {
    let code = "interface Empty {}";
    let rust = gourd_codegen::transpile_go_text(code);
    let rust_str = quote::quote! { #rust }.to_string();
    assert!(rust_str.contains("trait Empty"), "expected 'trait Empty' in: {}", rust_str);
}
