# RFC 002: Go Receiver Functions (impl blocks)

**Status**: NOT YET IMPLEMENTED
**Priority**: 2 (High)
**Complexity**: Medium

## Goal

Transform Go receiver method syntax into Rust `impl` blocks:

```go
go! {
    func (f Foo) Bar(z int) int {
        f.x + z
    }
}
// Transpiles to:
impl Foo {
    fn Bar(&self, z: i32) -> i32 {
        self.x + z
    }
}
```

## Implementation

1. **Add `GoReceiver` struct** with `name: Ident` and `type: syn::Type` fields.
2. **Extend `GoFn`** with an optional `receiver: Option<GoReceiver>` field.
3. **Parse the receiver** in `GoFn::parse` — after parsing the method name but before inputs, detect optional `(name Type)`.
4. **Generate `impl TypeName { fn method(&self, ...) { ... } }`** when a receiver is present, vs bare `fn` otherwise.

### Edge cases

- `(f *foo)` (pointer receiver) → `impl foo { fn method(&self, ...) { ... } }`
- `(foo Foo)` (value receiver) → `impl Foo { fn method(&self, ...) { ... } }` (same, Rust idiom)
- Multiple receivers → error (Go disallows, we should too)
