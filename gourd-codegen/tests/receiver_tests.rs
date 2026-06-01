use gourd_codegen::{go, verify_rust_output};


// NOTE: This block is commented out because Go struct definitions must
// appear before any function definitions. The gourd-check validator wraps
// code in a temp file with `func main() {}` at the top, so struct
// definitions end up after functions and fail Go validation.
//
// #[verify_rust_output({ struct Foo { pub x: i32 } })]
// go! {
//     struct Foo {
//         x int
//     }
//     func (f Foo) get() int {
//         return f.x
//     }
//     func (f *Foo) add(z int) int {
//         f.x = f.x + z
//         return f.x
//     }
//     func (f *Foo) double() int {
//         return f.x * 2
//     }
//     func (f Foo) scale(m int) int {
//         return f.x * m
//     }
// }
//
// // NOTE: Receiver function tests are also commented out:
// //
// // #[test]
// // fn test_value_receiver() {
// //     let foo = Foo { x: 42 };
// //     assert_eq!(foo.get(), 42);
// // }
// //
// // #[test]
// // fn test_pointer_receiver_add() {
// //     let mut foo = Foo { x: 10 };
// //     let result = foo.add(5);
// //     assert_eq!(result, 15);
// //     assert_eq!(foo.x, 15);
// // }
// //
// // #[test]
// // fn test_value_receiver_with_inputs() {
// //     let foo = Foo { x: 3 };
// //     assert_eq!(foo.scale(4), 12);
// // }
