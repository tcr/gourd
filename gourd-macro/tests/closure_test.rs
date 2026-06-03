//! Test Go → Rust closure transpilation.

#[test]
fn test_closure_no_params() {
    use gourd_macro::go;

    // Go: `func() { body }`
    // Rust: `|| { body }`
    go! {
        func main() int {
            f := func() int { return 42 }
            return f()
        }
    }
}

#[test]
fn test_closure_with_params() {
    use gourd_macro::go;

    // Go: `func(x int, y int) int { return x + y }`
    // Rust: `|x: i32, y: i32| -> i32 { x + y }`
    go! {
        func main() int {
            add := func(x int, y int) int { return x + y }
            return add(1, 2)
        }
    }
}

#[test]
fn test_closure_no_return() {
    use gourd_macro::go;

    // Go: `func() { ... }` — no return type
    // Rust: `|| { ... }`
    go! {
        func main() int {
            f := func() { return 0 }
            f()
            return 0
        }
    }
}

#[test]
fn test_closure_no_return_type() {
    use gourd_macro::go;

    go! {
        func main() int {
            f := func() { return 0 }
            f()
            return 0
        }
    }
}

#[test]
fn test_closure_with_slice_param() {
    use gourd_macro::go;

    go! {
        func main() int {
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
}
