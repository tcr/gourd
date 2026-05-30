use gourd_codegen::{go, verify_rust_output};


#[verify_rust_output({ VERIFY_MISMATCH })]
#[verify_rust_output({ fn go_minimal ( n : i32 ) - > String { match n { 1 = > { : : std : : string : : String : : from ( "one" ) } , 2 = > { : : std : : string : : String : : from ( "two" ) } , _ = > { : : std : : string : : String : : from ( "other" ) } } } })]
go! {
    fn go_minimal(n int) string {
        switch n {
        case 1:
            "one"
        case 2:
            "two"
        default:
            "other"
        }
    }
}

#[test]
fn test_minimal_switch() {
    assert_eq!(go_minimal(1), "one");
    assert_eq!(go_minimal(2), "two");
    assert_eq!(go_minimal(3), "other");
}
