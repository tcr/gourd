use proc_macro2::TokenStream;
use syn::{Expr, parse2};

#[test]
fn test_std_copy_parsing() {
    // Test how syn parses `std::copy(dst, src)`
    let code = "std :: copy ( dst , src )";
    let tokens: TokenStream = code.parse().unwrap();
    
    // Try parsing as Expr
    match parse2::<Expr>(tokens) {
        Ok(expr) => {
            println!("Parsed as Expr: {}", quote::quote!(#expr));
            match &expr {
                Expr::Call(call) => {
                    println!("  → Expr::Call");
                    println!("  func: {}", quote::quote!(#call.func));
                }
                Expr::MethodCall(method) => {
                    println!("  → Expr::MethodCall");
                    println!("  receiver: {}", quote::quote!(#method.receiver));
                    println!("  method: {}", method.method);
                }
                Expr::Path(path) => {
                    println!("  → Expr::Path");
                    println!("  path: {}", quote::quote!(#path));
                }
                _ => {
                    println!("  → Other: {:?}", std::mem::discriminant(&expr));
                }
            }
        }
        Err(e) => {
            println!("Parse error: {:?}", e);
        }
    }
}
