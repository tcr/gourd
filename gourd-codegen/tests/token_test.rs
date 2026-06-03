//! Token level tests for transpilation.

use gourd_codegen::transpile_go;
use proc_macro2::TokenStream;
use quote::quote;
use syn;

#[test]
fn test_basic_output() {
    let ts: TokenStream = quote! {
        func goAbs(n int) int {
            ret := n
            if n < 0 { ret = -n }
            return ret
        }
    };
    
    let output = transpile_go(ts);
    let output_str = output.to_string();
    let _file: syn::File = syn::parse_str(&output_str)
        .unwrap_or_else(|e| panic!("Failed to parse output as syn::File: {}\nOutput: {}", e, output_str));
}
