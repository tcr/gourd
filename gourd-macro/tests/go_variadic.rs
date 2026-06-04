use gourd_macro::go;

// ── Variadic parameter: `nums ...int` → `nums: &[i32]` ─────────────────

go! {
    func goSum(nums ...int) int {
        total := 0
        for i, num := range nums {
            if i >= 0 {
                total = total + num
            }
        }
        return total
    }
}

// ── Variadic with count: `count ...int` → `count: &[i32]` ─────────────

go! {
    func goCount(nums ...int) int {
        count := len(nums)
        return count
    }
}

// ── Mixed params with variadic: `min int, nums ...int` ────────────────

go! {
    func goFilter(min int, nums ...int) int {
        total := 0
        for i, num := range nums {
            if num > min {
                total = total + num
            }
        }
        return total
    }
}

#[test]
fn test_variadic_sum_works() {
    let data = vec![1, 2, 3, 4, 5];
    let result = goSum(&data);
    assert_eq!(result, 15); // 1+2+3+4+5
}

#[test]
fn test_variadic_count_works() {
    let data = vec![1, 2, 3, 4, 5];
    let result = goCount(&data);
    assert_eq!(result, 5); // length of slice
}

#[test]
fn test_variadic_filter_works() {
    let data = vec![1, 5, 3, 8, 2];
    let result = goFilter(3, &data);
    assert_eq!(result, 13); // 5+8 (numbers > 3)
}
