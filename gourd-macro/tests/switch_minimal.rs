//! Test Go → Rust minimal switch (no-selector) transpilation.

use gourd_macro::go;

go! {
    func goMinimal(n int) string {
        return switch n {
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
    assert_eq!(goMinimal(1), "one");
    assert_eq!(goMinimal(2), "two");
    assert_eq!(goMinimal(3), "other");
}
