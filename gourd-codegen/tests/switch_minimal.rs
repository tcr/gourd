use gourd_codegen::go;

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
