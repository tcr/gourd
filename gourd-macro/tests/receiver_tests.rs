//! Test Go → Rust receiver (impl block) transpilation.

use gourd_macro::go;

// Module-level structs and impls — go! at item level.
go! {
    struct Bar {
        value int
    }
    func (b *Bar) add(z int) int {
        b.value = b.value + z
        return b.value
    }
}

go! {
    struct Baz {
        n int
    }
    func (b Baz) scale(m int) int {
        return b.n * m
    }
}

go! {
    struct Qux {
        data int
    }
    func (q *Qux) double() int {
        return q.data * 2
    }
}

#[test]
fn test_pointer_receiver_add() {
    let mut bar = Bar { value: 10 };
    let result = bar.add(5);
    assert_eq!(result, 15);
    assert_eq!(bar.value, 15);
}

#[test]
fn test_value_receiver_scale() {
    let baz = Baz { n: 3 };
    assert_eq!(baz.scale(4), 12);
}

#[test]
fn test_pointer_receiver_double() {
    let mut qux = Qux { data: 7 };
    assert_eq!(qux.double(), 14);
}
