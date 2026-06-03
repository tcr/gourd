use gourd_macro::go;

// ── Multi-expression switch case: `case 1, 2, 3: "small"` ─────────────

go! {
    func goDayName(d int) string {
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
    assert_eq!(goDayName(1), "weekday");
    assert_eq!(goDayName(2), "weekday");
    assert_eq!(goDayName(3), "weekday");
    assert_eq!(goDayName(4), "almost_weekend");
    assert_eq!(goDayName(5), "almost_weekend");
    assert_eq!(goDayName(6), "weekend");
    assert_eq!(goDayName(7), "weekend");
    assert_eq!(goDayName(0), "weekend");
    assert_eq!(goDayName(-1), "invalid");
    assert_eq!(goDayName(100), "invalid");
}
