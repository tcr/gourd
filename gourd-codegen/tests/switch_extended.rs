use gourd_codegen::go;

// Switch without selector — case expressions use identifiers
go! {
    fn no_selector(ok bool, ready bool) string {
        switch {
        case ok:
            "ok"
        case ready:
            "ready"
        default:
            "none"
        }
    }
}

// Integer literal case with string result
go! {
    fn int_case(x int) string {
        switch x {
        case 42:
            "the answer"
        case 0:
            "zero"
        default:
            "other"
        }
    }
}

#[test]
fn test_no_selector() {
    assert_eq!(no_selector(true, false), "ok");
    assert_eq!(no_selector(false, true), "ready");
    assert_eq!(no_selector(false, false), "none");
}

#[test]
fn test_int_case() {
    assert_eq!(int_case(42), "the answer");
    assert_eq!(int_case(0), "zero");
    assert_eq!(int_case(99), "other");
}
