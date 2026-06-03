use gourd_macro::go;

// ── Continue statement ────────────────────────────────────────────────
// Continue skips to the next iteration when a condition is met.

go! {
    func go_skip_odd(data []int) int {
        ret := 0
        i := 0
        while i < len(data) {
            v := data[int(i)]
            i = i + 1  // Always increment first
            if v % 2 != 0 {
                continue  // Then skip odd values
            }
            ret = ret + 1
        }
        return ret
    }
}

#[test]
fn test_continue_skips() {
    let data = vec![1, 2, 3, 4, 5, 6];
    let result = go_skip_odd(&data);
    assert_eq!(result, 3); // 2, 4, 6 are even (3 values)
}
