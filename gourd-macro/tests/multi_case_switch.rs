use gourd_macro::go;

// ── Multi-expression switch case: `case 1, 2, 3: "small"` ─────────────

go! {
    func go_day_name(d int) string {
        switch d {
        case 1, 2, 3:
            return "weekday"
        case 4, 5:
            return "almost_weekend"
        case 6, 7, 0:
            return "weekend"
        default:
            return "invalid"
        }
    }
}

#[test]
fn test_multi_case_switch() {
    assert_eq!(go_day_name(1), "weekday");
    assert_eq!(go_day_name(2), "weekday");
    assert_eq!(go_day_name(3), "weekday");
    assert_eq!(go_day_name(4), "almost_weekend");
    assert_eq!(go_day_name(5), "almost_weekend");
    assert_eq!(go_day_name(6), "weekend");
    assert_eq!(go_day_name(7), "weekend");
    assert_eq!(go_day_name(0), "weekend");
    assert_eq!(go_day_name(-1), "invalid");
    assert_eq!(go_day_name(100), "invalid");
}
