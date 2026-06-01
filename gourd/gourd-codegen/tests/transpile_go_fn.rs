//! Transpile unit tests for free Go functions.
//!
//! These tests call `gourd_codegen_core::transpile_go()` directly with
//! Go-like TokenStreams and compare the output TokenStream against expected
//! Rust tokens (using `quote!` to build both sides).

use gourd_codegen_core::transpile_go;
use proc_macro2::TokenStream;
use quote::quote;

/// Build a Go-like function signature token stream using `quote!`.
/// The tokens aren't valid Rust syntax, but that's fine — the proc-macro
/// receives raw tokens and transpiles them.
fn go_fn(tokens: TokenStream) -> TokenStream {
    tokens
}

/// Compare a transpiled TokenStream to an expected Rust TokenStream.
///
/// Both are parsed as `syn::File` for structural comparison of
/// function signatures (name, params, return types).
fn assert_transpile_matches(input: TokenStream, expected: TokenStream) {
    let output = transpile_go(input);
    let output_str = output.to_string();

    // Parse the expected tokens as syn::File
    let expected_file: syn::File = syn::parse_quote!(#expected);

    // Parse the transpiled output as syn::File
    let output_file: syn::File =
        syn::parse_str(&output_str).unwrap_or_else(|e| {
            panic!(
                "Failed to parse transpiled output as syn::File.\nExpected:\n  {}\nActual output:\n  {}\nError: {}",
                expected, output_str, e
            );
        });

    // Collect functions
    let expected_items: Vec<_> = expected_file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(f) => Some(f),
            _ => None,
        })
        .collect();
    let output_items: Vec<_> = output_file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(f) => Some(f),
            _ => None,
        })
        .collect();

    assert_eq!(
        expected_items.len(),
        output_items.len(),
        "Function count mismatch.\nExpected: {} functions\nActual: {} functions\nExpected:\n  {}\nActual:\n  {}",
        expected_items.len(),
        output_items.len(),
        quote!(#expected),
        output_str
    );

    // Match each expected function to its transpiled counterpart
    for (exp_fn, act_fn) in expected_items.iter().zip(output_items.iter()) {
        assert_eq!(
            exp_fn.sig.ident.to_string(),
            act_fn.sig.ident.to_string(),
            "Function name mismatch.\nExpected: {}\nActual: {}",
            exp_fn.sig.ident,
            act_fn.sig.ident
        );

        assert_eq!(
            exp_fn.sig.inputs.len(),
            act_fn.sig.inputs.len(),
            "Parameter count mismatch for '{}'. Expected: {} params, Actual: {} params",
            exp_fn.sig.ident,
            exp_fn.sig.inputs.len(),
            act_fn.sig.inputs.len(),
        );

        for (exp_pat, act_pat) in exp_fn.sig.inputs.iter().zip(act_fn.sig.inputs.iter()) {
            assert_eq!(
                quote::quote!(#exp_pat).to_string(),
                quote::quote!(#act_pat).to_string(),
                "Parameter mismatch for '{}'.\nExpected: {}\nActual: {}",
                exp_fn.sig.ident,
                quote::quote!(#exp_pat),
                quote::quote!(#act_pat)
            );
        }

        match (&exp_fn.sig.output, &act_fn.sig.output) {
            (syn::ReturnType::Default, syn::ReturnType::Default) => {}
            (syn::ReturnType::Type(_, exp_ret), syn::ReturnType::Type(_, act_ret)) => {
                assert_eq!(
                    quote::quote!(#exp_ret).to_string(),
                    quote::quote!(#act_ret).to_string(),
                    "Return type mismatch for '{}'.\nExpected: {}\nActual: {}",
                    exp_fn.sig.ident,
                    quote::quote!(#exp_ret),
                    quote::quote!(#act_ret)
                );
            }
            (_, _) => {
                panic!(
                    "Return type mismatch for '{}'.",
                    exp_fn.sig.ident
                );
            }
        }
    }
}

// ─── Basic tests (simple function signatures) ───

#[test]
fn test_basic_return() {
    let input = go_fn(quote! { fn go_add() int { 42 } });
    assert_transpile_matches(input, quote! { fn go_add() -> i32 { 42 } });
}

#[test]
fn test_basic_params() {
    let input = go_fn(quote! { fn go_sum(a i32, b i32) i32 { a + b } });
    assert_transpile_matches(input, quote! { fn go_sum(a: i32, b: i32) -> i32 { a + b } });
}

#[test]
fn test_if_return() {
    let input = go_fn(quote! {
        fn go_abs(n int) int {
            let mut ret = n;
            if n < 0 { ret = -n; }
            ret
        }
    });
    assert_transpile_matches(
        input,
        quote! {
            fn go_abs(n: i32) -> i32 {
                let mut ret = n;
                if n < 0 { ret = -n; }
                ret
            }
        },
    );
}

#[test]
fn test_bool_return() {
    let input = go_fn(quote! { fn is_even(n int) bool { n % 2 == 0 } });
    assert_transpile_matches(input, quote! { fn is_even(n: i32) -> bool { n % 2 == 0 } });
}

#[test]
fn test_no_return() {
    let input = go_fn(quote! { fn go_incr() i32 { 42 } });
    assert_transpile_matches(input, quote! { fn go_incr() -> i32 { 42 } });
}

// ─── Multi-return tests ───

#[test]
fn test_multi_return() {
    let input = go_fn(quote! {
        fn go_divmod(n int, d int) (int, int) {
            (n / d, n % d)
        }
    });
    assert_transpile_matches(
        input,
        quote! {
            fn go_divmod(n: i32, d: i32) -> (i32, i32) {
                (n / d, n % d)
            }
        },
    );
}

#[test]
fn test_mixed_tuple_return() {
    let input = go_fn(quote! {
        fn go_format(n int) (int, string) {
            (n, String::from("hello"))
        }
    });
    assert_transpile_matches(
        input,
        quote! {
            fn go_format(n: i32) -> (i32, String) {
                (n, String::from("hello"))
            }
        },
    );
}

#[test]
fn test_triple_return() {
    let input = go_fn(quote! {
        fn go_triple(a int, b int) (int, int, string) {
            (a + b, a * b, String::from("pair"))
        }
    });
    assert_transpile_matches(
        input,
        quote! {
            fn go_triple(a: i32, b: i32) -> (i32, i32, String) {
                (a + b, a * b, String::from("pair"))
            }
        },
    );
}

// ─── Parameter type tests ───

#[test]
fn test_string_param() {
    let input = go_fn(quote! { fn go_len(s string) i32 { s.len() as i32 } });
    assert_transpile_matches(input, quote! { fn go_len(s: String) -> i32 { s.len() as i32 } });
}

#[test]
fn test_slice_type_param() {
    let input = go_fn(quote! { fn go_slice_len(a []int) i32 { a.len() as i32 } });
    assert_transpile_matches(input, quote! { fn go_slice_len(a: &[i32]) -> i32 { a.len() as i32 } });
}

#[test]
fn test_param_grouping() {
    let input = go_fn(quote! { fn go_shorthand(a, b, c int) int { a + b + c } });
    assert_transpile_matches(
        input,
        quote! { fn go_shorthand(a: i32, b: i32, c: i32) -> i32 { a + b + c } },
    );
}

// ─── Slice literal body tests ───

#[test]
fn test_slice_literal_body() {
    let input = go_fn(quote! { fn go_slice_literal() Vec<int> { []int{ 1, 2, 3 } } });
    assert_transpile_matches(
        input,
        quote! {
            fn go_slice_literal() -> Vec<i32> {
                <[_]>::into_vec(::alloc::boxed::box_new([1, 2, 3]))
            }
        },
    );
}

#[test]
fn test_slice_literal_empty_body() {
    let input = go_fn(quote! { fn go_slice_literal_empty() Vec<int> { []int{} } });
    assert_transpile_matches(
        input,
        quote! { fn go_slice_literal_empty() -> Vec<i32> { ::alloc::vec::Vec::new() } },
    );
}

#[test]
fn test_slice_literal_type_inferred_body() {
    let input = go_fn(quote! {
        fn go_slice_literal_type_inferred() Vec<int> {
            []{ 2, 3, 4 }
        }
    });
    assert_transpile_matches(
        input,
        quote! {
            fn go_slice_literal_type_inferred() -> Vec<i32> {
                <[_]>::into_vec(::alloc::boxed::box_new([2, 3, 4]))
            }
        },
    );
}

// ─── String builtin test ───

#[test]
fn test_string_builtin() {
    let input = go_fn(quote! { fn go_str(bytes []byte) string { string(bytes) } });
    assert_transpile_matches(
        input,
        quote! {
            fn go_str(bytes: &[u8]) -> String {
                std::str::from_utf8(&bytes).unwrap_or("").to_string()
            }
        },
    );
}

// ─── Map literal body tests ───

#[test]
fn test_map_literal_empty_body() {
    let input = go_fn(quote! { fn go_map_literal_empty() bool { let m = map<int,string>{}; m.is_empty() } });
    assert_transpile_matches(
        input,
        quote! {
            fn go_map_literal_empty() -> bool {
                let m = std::collections::HashMap::<i32, String>::default();
                m.is_empty()
            }
        },
    );
}

#[test]
fn test_int_map_body() {
    let input = go_fn(quote! {
        fn go_int_map() string {
            let m = map<int,string>{ 1: "one", 2: "two" };
            m.get(2).unwrap().clone()
        }
    });
    assert_transpile_matches(
        input,
        quote! {
            fn go_int_map() -> String {
                {
                    let mut m = std::collections::HashMap::<i32, String>::new();
                    m.insert(1, ::std::string::String::from("one"));
                    m.insert(2, ::std::string::String::from("two"));
                    m
                }.get(&2).unwrap().clone()
            }
        },
    );
}
