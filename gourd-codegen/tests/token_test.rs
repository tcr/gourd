use proc_macro2::TokenStream;
use std::str::FromStr;
use syn::parse2;
use syn::token;

#[test]
fn test_lt_peek_with_joint_spacing() {
    // Original tokenization has Joint spacing
    let ts = TokenStream::from_str("ch <- value").unwrap();
    println!("Original token stream: {:?}", ts);
    
    // Try to parse < from the original token stream
    // We need to parse the first identifier first, then check for <
    let expr: syn::Expr = parse2(ts.clone()).unwrap();
    println!("Expression: {:?}", quote::quote! { #expr });
}

#[test]
fn test_lt_detection_in_parse2() {
    // Try to parse < directly from a token stream
    let ts = TokenStream::from_str("< - value").unwrap();
    println!("Token stream for '< - value': {:?}", ts);
    
    // Try to parse Lt
    let result: Result<token::Lt, _> = parse2(ts);
    println!("Parse Lt result: {:?}", result.is_ok());
}
