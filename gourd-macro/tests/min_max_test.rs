use gourd_macro::go;

go! {
    func goMin(a int, b int) int {
        return min(a, b)
    }
}

go! {
    func goMax(a int, b int) int {
        return max(a, b)
    }
}

#[test]
fn test_min() {
    assert_eq!(goMin(1, 2), 1);
    assert_eq!(goMin(3, 2), 2);
}

#[test]
fn test_max() {
    assert_eq!(goMax(1, 2), 2);
    assert_eq!(goMax(3, 2), 3);
}
