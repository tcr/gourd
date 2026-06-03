use gourd_macro::go;

// ── For range with two variables ──────────────────────────────────────

go! {
    func go_sum_slice(data []int) int {
        sum := 0
        for i, v := range data {
            if i >= 0 && v > 0 {
                sum = sum + v
            }
        }
        return sum
    }
}

// ── For range with single variable ────────────────────────────────────

go! {
    func go_count_positive(data []int) int {
        count := 0
        for i := range data {
            if data[i] > 0 {
                count = count + 1
            }
        }
        return count
    }
}

// ── For range without collecting values ───────────────────────────────

go! {
    func go_skip_first(data []int) int {
        for range data {
            // skip, just iterate
        }
        return len(data)
    }
}

#[test]
fn test_for_range_double() {
    let data = vec![1, -2, 3, -4, 5];
    let result = go_sum_slice(&data);
    assert_eq!(result, 9); // 1 + 3 + 5
}

#[test]
fn test_for_range_single() {
    let data = vec![1, -2, 3, -4, 5];
    let result = go_count_positive(&data);
    assert_eq!(result, 3); // 1, 3, 5 are positive
}

#[test]
fn test_for_range_skip() {
    let data = vec![1, 2, 3, 4, 5];
    let result = go_skip_first(&data);
    assert_eq!(result, 5);
}
