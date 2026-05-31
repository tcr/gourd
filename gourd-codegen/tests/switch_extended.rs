use gourd_codegen::{go, verify_rust_output};

// Switch without selector — case expressions use identifiers

#[verify_rust_output({ fn no_selector(ok: bool, ready: bool) -> String { if ok { ::std::string::String::from("ok") } else if ready { ::std::string::String::from("ready") } else { ::std::string::String::from("none") } } })]
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

#[verify_rust_output({ fn int_case(x: i32) -> String { match x { 42 => { ::std::string::String::from("the answer") } , 0 => { ::std::string::String::from("zero") } , _ => { ::std::string::String::from("other") } } } })]
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
