# RFC 005: GoGc — Arc-based Garbage-Collected Pointer Type

**Status**: ✅ IMPLEMENTED
**Priority**: 2 (High)
**Complexity**: Low

## Goal

Provide a Go-style memory management primitive for code transpiled through Gourd:

```rust
use gourd::GoGc;

struct Point { x: i32, y: i32 }

let p = GoGc::new(Point { x: 1, y: 2 });
let q = GoGc::clone(&p);

// p.x and q.x both access the same heap-allocated Point
assert_eq!(p.x, 1);
assert_eq!(q.x, 1);

// Deallocation happens when the last reference drops
drop(p);
// q still valid, memory not yet freed (strong_count = 1)

matcGoGc::try_unwrap(q) {
    Ok(pt) => println!("Point: {{ x: {}, y: {} }}", pt.x, pt.y),
    Err(_) → panic!("refcount > 1"),
}
```

The key properties:

| Go semantics | GoGc behavior |
|---|---|
| Go variables assigned to structs always receive heap memory | `GoGc::new(T)` allocates on the heap |
| Go pointers are shared; copying increases refcount | `GoGc::clone()` increments Arc refcount |
| Go's collector reclaims unreachable objects | Last `GoGc` dropped → memory freed |
| No manual memory management | No manual `.free()` or `.dispose()` needed |

## Background

Go programs manage memory through a concurrent mark-and-sweep garbage collector.
When programming in Go, developers never write `malloc` or `free` — the runtime
handles allocation, reference counting, and collection.

Rust's philosophy is fundamentally different: ownership + borrowing at
compile time eliminates the need for a garbage collector entirely. However,
for Go developers migrating to Rust (the target of Gourd), this is a significant
mental model shift.

**This RFC proposes `GoGc<T>`**, a thin runtime wrapper around `Arc<T>` that
gives Go developers a familiar memory model within Rust's type system.

## Mapping Rules

| Go concept | GoGc behavior |
|---|---|
| `new(T)` — Go predeclared function | `GoGc::new(value: T)` → heap allocation, refcount = 1 |
| Pointer copy (implicit) | `GoGc::clone(&gc)` → refcount + 1 |
| GC cycle (indirect deallocation) | Last `GoGc` goes out of scope → `Arc` drops → memory freed |
| "Is this the only reference?" | `GoGc::try_unwrap(gc)` → `Ok(T)` if refcount == 1, else `Err(GoGc<T>)` |
| Reference count introspection | `GoGc::strong_count(&gc)` → `usize` |

## Current Implementation

**Location:** `gourd/src/go_gc.rs` (integrated into the `gourd` library crate)

```rust
pub struct GoGc<T: 'static + ?Sized> {
    inner: Arc<T>,
}

impl<T: 'static> GoGc<T> {
    pub fn new(value: T) → Self;
    pub fn into_inner(self) → Arc<T>;
    pub fn try_unwrap(self) → Result<T, GoGc<T>>;
}

impl<T: ?Sized> GoGc<T> {
    pub fn strong_count(&self) → usize;
}

impl<T: ?Sized> Deref for GoGc<T>;
impl<T: ?Sized> Clone for GoGc<T>;
impl<T: PartialEq + ?Sized> PartialEq for GoGc<T>;
impl<T: Eq + ?Sized> Eq for GoGc<T>;
impl<T: Display + ?Sized> Display for GoGc<T>;
impl Debug for GoGc<T>;
```

### Key design decisions

1. **`'static` bound**: All `GoGc<T>` values carry `'static`, matching Go's guarantee that heap objects outlive function scope. This means `GoGc<T>` cannot hold borrowed data.

2. **`?Sized`**: Supports unsized types (`GoGc<[T]>`, `GoGc<str>`) via the inner `Arc`.

3. **`Deref`**: Transparent field/method access through `gc.field` syntax. No explicit `.inner` field access on user code.

4. **No automatic `Drop` plumbing**: `GoGc` relies on `Arc::drop` (called by the compiler-generated drop) for deallocation. The implementation does not define an explicit `Drop` impl — this keeps the type semantically aligned with `Arc`.

### public API surface

| Method | Type | Behavior |
|---|---|---|
| `GoGc::new(value)` | `new` | Allocates `value` on the heap, returns `GoGc` with refcount = 1 |
| `GoGc::clone(&gc)` | `Clone` | Returns a new `GoGc` sharing the same `Arc<T>`, refcount + 1 |
| `gc.strong_count()` | `usize` | Returns the current strong reference count |
| `GoGc::into_inner(gc)` | `Arc<T>` | Consumes `GoGc`, returns inner `Arc<T>` (count unchanged) |
| `GoGc::try_unwrap(gc)` | `Result<T, GoGc<T>>` | Returns `Ok(inner_value)` if refcount == 1, else `Err(gc)` (count unchanged |
| `gc.field` | (via `Deref`) | Transparently accesses the wrapped value's fields/methods |

## Limitations

### No cycle detection

`GoGc<T>` uses `Arc<T>` internally, which cannot break cycles. Two `GoGc` values pointing to each other form a cycle whose refcount never reaches 0, and memory is never reclaimed.

This matches Go's own behavior: Go's mark-and-sweep collector **cannot collect cycles of structs that reference each other** (the GC doesn't have graph-traversal that reaches back through cycles).

> **Example:** `GoGc::new(Node { next: None })` and `GoGc::new(Node { next: Some(only) })` form an uncollectable cycle.

**Mitigation for future work**: A `WeakGoGc<T>` cousin type (wrapping `std::sync::Weak<T>`) could be added to allow users to break cycles, similar to how `Weak<T>` works in Rust.

### `'static` lifetime

`GoGc<T>` requires `T: 'static`. This means:

```go
// ❌ This does NOT compile:
let gc: GoGc<String> = {
    let owned = String::from("owned");
    GoGc::new(owned)  // err: `owned` does not live long enough
};

// ✅ Use a `static` or heap-allocated constant:
static HELLO: &str = "hello";
let gc: GoGc<&str> = GoGc::new(HELLO);
```

This is by design: Go heap objects always outlive the scope that created them.

### No interop with Rust's borrow checker

`GoGc<T>` itself doesn't play nice with Rust's borrow checker. Users won't typically hold `&GoGc<T>` while also holding a mutable reference to the inner value — Go programs are not expected to do this either.

## Behavior in Transpiled Code

The transpiler emits `GoGc<T>` as a valid Rust type identifier. **No changes to the transpiler** are needed — `GoGc<T>` is simply a Rust struct name that users import.

```go
// User writes (inside a go_expr! block):
let obj := GoGc::new(MyStruct { x: 42 });
let cloned := GoGc::clone(obj);
```

The transpiler emits (identical to the above, since `GoGc::new` and `GoGc::clone`
are valid Rust function calls).

Users import via:

```rust
use gourd::GoGc;
```

## Testing

8 integration tests in `gourd/tests/gc_tests.rs`:

- `test_new_creates_single_reference` — `GoGc::new` returns refcount 1
- `test_clone_increments_reference_count` — `GoGc::clone` increments count
- `test_deref_access_fields` — field access via transparent `Deref`
- `test_into_inner_preserves_reference_count` — `into_inner` consumes `GoGc` without dropping count
- `test_try_unwrap_succeeds_with_single_reference` — succeeds when count = 1
- `test_try_unwrap_fails_with_multiple_references` — fails when count > 1
- `test_eq_and_partial_cmp` — equality comparison works correctly
- `test_display` — formatting flows through to inner value

## Future Directions

### `WeakGoGc<T>` (breaking cycles)

Add a `WeakGoGc<T>` type that wraps `std::sync::Weak<T>`, allowing users to express "weak" references that don't prevent deallocation:

```rust
struct Node {
    value: i32,
    next: Option<WeakGoGc<Node>>,  // weak reference: doesn't prevent collection
}
```

### Collector notifications

Te `Drop` impl (or explicitly-defined `Drop`) can log reference count changes:

```rust
// Development/debugging: emit "GC: Node[T] refcount dropped to 0"
```

### Custom allocators

Replace `Arc<T>` with a custom mark-and-sweep allocator once Go's GC is fully documented:

```rust
// Replacement (future):
impl<T: ?Sized> GoGc<T> {
    inner: CustomAllocator<T>,  // Mark-and-sweep instead of Arc
}
```

The API surface does not change — only the internal storage mechanism.

## References

- [FEATURE ROADMAP](../ROADMAP.md)
- [Go Runtime: Garbage Collector](https://golang.org/doc/gogc)
- [Rust Review](https://blog.stalkr.net/2016/06/smemcptr-fgogc.html)
- RFC 004: Go Error Handling → Rust Result
- Rust documentation on `std::sync::Arc`
