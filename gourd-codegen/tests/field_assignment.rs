use gourd_codegen::go;

// ── Struct definition ────────────────────────────────────────────────

go! {
    struct Counter {
        count int
        max   int
    }
}

// ── Receiver: pointer, no params ─────────────────────────────────────

go! {
    func (c *Counter) increment() int {
        c.count = c.count + 1
        return c.count
    }
}

// ── Receiver: pointer, with params ───────────────────────────────────

go! {
    func (c *Counter) add(n int) int {
        c.count = c.count + n
        if c.count > c.max {
            c.count = c.max
        }
        return c.count
    }
}

// ── Receiver: value, no params ───────────────────────────────────────

go! {
    func (c Counter) get() int {
        return c.count
    }
}

#[test]
fn test_field_assignment() {
    let mut c = Counter { count: 0, max: 10 };
    assert_eq!(c.increment(), 1);
    assert_eq!(c.increment(), 2);
    assert_eq!(c.get(), 2);
    assert_eq!(c.add(5), 7);
    assert_eq!(c.get(), 7);
    // Test capping at max
    assert_eq!(c.add(10), 10); // capped at max=10
    assert_eq!(c.get(), 10);
}
