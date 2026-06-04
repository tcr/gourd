use gourd_macro::go;

// Test: closure with len() builtin
go! {
    func goClosureWithLen() int {
        f := func(arr []int) int {
            if len(arr) > 0 {
                return arr[0]
            }
            return 0
        }
        data := []int{1, 2, 3}
        return f(&data)
    }
}

// Test: closure with [] indexing
go! {
    func goClosureWithIndex() int {
        f := func(arr []int) int {
            return arr[1]
        }
        data := []int{10, 20, 30}
        return f(&data)
    }
}

#[test]
fn test_closure_len_builtin() {
    assert_eq!(goClosureWithLen(), 1);
}

#[test]
fn test_closure_index_builtin() {
    assert_eq!(goClosureWithIndex(), 20);
}
