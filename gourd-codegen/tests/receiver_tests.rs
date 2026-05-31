use gourd_codegen::{go, verify_rust_output};


#[verify_rust_output({ struct Foo { pub x: i32 } })]
go! {
    struct Foo {
        x int
    }
}


#[verify_rust_output({ impl Foo { fn get(&self) -> i32 { self.x } } })]
go! {
    func (f Foo) get() int {
        f.x
    }
}


#[verify_rust_output({ impl Foo { fn add(&mut self, z: i32) -> i32 { self.x = self.x + z; self.x } } })]
go! {
    func (f *Foo) add(z int) int {
        f.x = f.x + z
        f.x
    }
}


#[verify_rust_output({ impl Foo { fn double(&mut self) -> i32 { self.x * 2 } } })]
go! {
    func (f *Foo) double() int {
        f.x * 2
    }
}


#[verify_rust_output({ impl Foo { fn scale(&self, m: i32) -> i32 { self.x * m } } })]
go! {
    func (f Foo) scale(m int) int {
        f.x * m
    }
}

#[test]
fn test_value_receiver() {
    let foo = Foo { x: 42 };
    assert_eq!(foo.get(), 42);
}

#[test]
fn test_pointer_receiver_add() {
    let mut foo = Foo { x: 10 };
    let result = foo.add(5);
    assert_eq!(result, 15);
    assert_eq!(foo.x, 15);
}

#[test]
fn test_value_receiver_with_inputs() {
    let foo = Foo { x: 3 };
    assert_eq!(foo.scale(4), 12);
}
