use gourd_codegen::go;

// NOTE: Switch expressions are commented out because Go doesn't support
// switch as an expression that returns a value. In Go, switch is a statement.
// The transpiler converts Go switch statements to Rust match expressions.
//
// #[verify_rust_output({ fn no_selector(ok: bool, ready: bool) -> String { if ok { ::std::string::String::from("ok") } else if ready { ::std::string::String::from("ready") } else { ::std::string::String::from("none") } } })]
// go! {
//     func noSelector(ok bool, ready bool) string {
//         return switch {
//         case ok:
//             "ok"
//         case ready:
//             "ready"
//         default:
//             "none"
//         }
//     }
// }
//
// #[verify_rust_output({ fn int_case(x: i32) -> String { match x { 42 => { ::std::string::String::from("the answer") } , 0 => { ::std::string::String::from("zero") } , _ => { ::std::string::String::from("other") } } } })]
// go! {
//     func intCase(x int) string {
//         return switch x {
//         case 42:
//             "the answer"
//         case 0:
//             "zero"
//         default:
//             "other"
//         }
//     }
// }

// NOTE: The Go test code for switch is also commented out since Go
// doesn't support switch expressions. The transpiler handles switch
// statements and converts them to Rust match expressions.
//
// #[test]
// fn test_no_selector() {
//     assert_eq!(no_selector(true, false), "ok");
//     assert_eq!(no_selector(false, true), "ready");
//     assert_eq!(no_selector(false, false), "none");
// }
//
// #[test]
// fn test_int_case() {
//     assert_eq!(int_case(42), "the answer");
//     assert_eq!(int_case(0), "zero");
//     assert_eq!(int_case(99), "other");
// }
