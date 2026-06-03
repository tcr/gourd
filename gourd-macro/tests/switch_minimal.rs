
// NOTE: Switch expressions are commented out because Go doesn't support
// switch as an expression that returns a value. In Go, switch is a statement.
// The transpiler converts Go switch statements to Rust match expressions.
//
// #[verify_rust_output({ VERIFY_MISMATCH })]
// #[verify_rust_output({ fn goMinimal(n: i32) -> String { match n { 1 => { ::std::string::String::from("one") } , 2 => { ::std::string::String::from("two") } , _ => { ::std::string::String::from("other") } } } })]
// go! {
//     func goMinimal(n int) string {
//         return switch n {
//         case 1:
//             "one"
//         case 2:
//             "two"
//         default:
//             "other"
//         }
//     }
// }
//
// #[test]
// fn test_minimal_switch() {
//     assert_eq!(goMinimal(1), "one");
//     assert_eq!(goMinimal(2), "two");
//     assert_eq!(goMinimal(3), "other");
// }
