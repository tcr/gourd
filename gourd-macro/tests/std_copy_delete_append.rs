use gourd_macro::go;

// ── std::copy test ──────────────────────────────────────────────────────────

go! {
    func goCopySlice() int {
        dst := []int{1, 2, 3, 4, 5}
        src := []int{10, 20}
        result := std::copy(dst, src)
        return result
    }
}

go! {
    func goCopyPartial() int {
        dst := []int{0, 0, 0, 0, 0}
        src := []int{1, 2, 3, 4, 5, 6}
        result := std::copy(dst, src)
        return result
    }
}

// ── std::delete test ────────────────────────────────────────────────────────

go! {
    func goDeleteFromMap() int {
        m := map[string]int{"a": 1, "b": 2, "c": 3}
        _deleted := std::delete(m, "b")
        if _deleted != nil {
            return 1
        } else {
            return 0
        }
    }
}

go! {
    func goDeleteNonexistent() int {
        m := map[string]int{"a": 1, "b": 2}
        _deleted := std::delete(m, "z")
        if _deleted != nil {
            return 1
        } else {
            return 0
        }
    }
}

// ── std::append test ────────────────────────────────────────────────────────

go! {
    func goAppendSingle() int {
        s := []int{1, 2, 3}
        result := std::append(s, 4)
        return len(result)
    }
}

go! {
    func goAppendMultiple() int {
        s := []int{1, 2, 3}
        result := std::append(s, 4, 5, 6)
        return len(result)
    }
}

go! {
    func goAppendEmpty() int {
        s := []int{1, 2, 3}
        result := std::append(s)
        return len(result)
    }
}

#[test]
fn test_std_copy_works() {
    let result = goCopySlice();
    assert_eq!(result, 2); // copied 2 elements from src
}

#[test]
fn test_std_copy_partial_works() {
    let result = goCopyPartial();
    assert_eq!(result, 5); // copied 5 elements (min of dst=5, src=6)
}

#[test]
fn test_std_delete_works() {
    let result = goDeleteFromMap();
    assert_eq!(result, 1); // deleted key, returned Some
}

#[test]
fn test_std_delete_nonexistent_works() {
    let result = goDeleteNonexistent();
    assert_eq!(result, 0); // key didn't exist, returned None
}

#[test]
fn test_std_append_single_works() {
    let result = goAppendSingle();
    assert_eq!(result, 4); // 3 + 1
}

#[test]
fn test_std_append_multiple_works() {
    let result = goAppendMultiple();
    assert_eq!(result, 6); // 3 + 3
}

#[test]
fn test_std_append_empty_works() {
    let result = goAppendEmpty();
    assert_eq!(result, 3); // no-op
}
