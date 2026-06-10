# Go → Rust Bridging Rubric

## Implementation Status (as of 2026-06-10)

### Category A: ✅ COMPLETE (104 functions across 12 packages)

All simple utilities have been implemented. These are reimplemented in Rust with no reason to bridge.

| Package | Functions | Status | File |
|---------|-----------|--------|------|
| `fmt` | 4+ | ✅ Done | `gourd/src/prelude/fmt_ops.rs` |
| `strings` | 19 | ✅ Done | `gourd/src/packages/strings_impl.rs` + `strings_ops.rs` |
| `bytes` | 7 | ✅ Done | `gourd/src/packages/bytes_ops.rs` |
| `math` | 14 | ✅ Done | `gourd/src/packages/math_ops.rs` |
| `json` | 2 | ✅ Done | `gourd/src/packages/json_ops.rs` |
| `time` | 4 | ✅ Done | `gourd/src/packages/time_impl.rs` |
| `os` | 11 | ✅ Done | `gourd/src/packages/os_impl.rs` |
| `io` | 2 | ✅ Done | `gourd/src/packages/io_ops.rs` |
| `byte` | 4 | ✅ Done | `gourd/src/packages/byte_ops.rs` |
| **`strconv`** | **13** | ✅ **NEW** | `gourd/src/packages/strconv_ops.rs` |
| **`unicode`** | **9** | ✅ **NEW** | `gourd/src/packages/unicode_ops.rs` |
| **`sort`** | **5** | ✅ **NEW** | `gourd/src/packages/sort_ops.rs` |
| **`log`** | **14** | ✅ **NEW** | `gourd/src/packages/log_impl.rs` |

**Test coverage:** 64 tests pass across all packages.

### Category B: ⏳ In Progress (0/7 implemented)

| Package | Status |
|---------|--------|
| `csv` | ❌ Not implemented |
| `url` | ❌ Not implemented |
| `math/big` | ❌ Not implemented (complex) |
| `text/tabwriter` | ❌ Not implemented |
| `text/template` | ❌ Not implemented (complex) |

### Category C: ⏳ Deferred (0/12 implemented)

Runtime-dependent libraries requiring rust2go/CGO bridging.

---

## The Three Options

| Option | What It Is | When It Applies |
|--------|-----------|-----------------|
| **① Reimplement in Rust** | Write a new Rust crate from scratch | There's already a good Rust alternative and the Go code is simple enough to reimplement |
| **② Transpile via Gourd** | Convert Go source to Rust | The code is algorithmic, deterministic, and small enough to transpile correctly |
| **③ Bridge via rust2go** | Call Go library at runtime through FFI | The code is runtime-dependent or too complex to reimplement/bridge |
| **④ Bridge via CGO** | Write hand-written cgo bindings | Needed for streaming/large data where rust2go's buffer approach fails |

---

## Decision Flow

```
Does this library have a native Rust equivalent?
  │
  ├─ Yes → Can it be implemented in < 500 lines of Rust?
  │         ├─ Yes → ① Reimplement in Rust
  │         └─ No  → ② Transpile via Gourd
  │
  └─ No → Is it algorithmic (deterministic, no runtime magic)?
           ├─ Yes → ② Transpile via Gourd
           └─ No  → ③ Bridge via rust2go
```

---

## The Three Criteria

### 1. Runtime Dependency — The paradigm vs. operations distinction

| Low Runtime Dependency | High Runtime Dependency |
|-----------------------|------------------------|
| Pure functions | Goroutine coordination |
| No GC interaction | GC-dependent lifetimes |
| Simple types | Complex type hierarchies |
| Deterministic | Non-deterministic timing |
| No side effects | Side effects on OS/resources |

### 2. Existing Rust Alternative

| Good Rust alternative exists | No Rust alternative |
|-----------------------------|---------------------|
| Reimplement (①) | Transpile (②) or Bridge (③) |

### 3. Implementation Size / Complexity

| Small (< 500 lines) | Large (> 500 lines) |
|---------------------|---------------------|
| Reimplement (①) or Transpile (②) | Bridge (③) |

---

## Complete stdlib Rubric

### Category A: Simple Utilities — Reimplement in Rust (①)

Native Rust equivalents exist. Small, well-tested. No reason to bridge or transpile.

**Status: ✅ ALL COMPLETE — 104 functions across 12 packages.**

| Library | Rust Equivalent | Status | Notes |
|---------|----------------|--------|-------|
| `fmt` | `format_args!` macro | ✅ Done | `gourd/src/prelude/fmt_ops.rs` |
| `strings` | `str::`, `String::` | ✅ Done | 19 functions in `strings_impl.rs` + `strings_ops.rs` |
| `bytes` | `&[u8]` | ✅ Done | 7 functions in `bytes_ops.rs` |
| `math` | `f64::`, `i32::` | ✅ Done | 14 functions in `math_ops.rs` |
| `sort` | `slice::sort()` | ✅ Done | 5 functions in `sort_ops.rs` (moved from strings) |
| `log` | `log` crate | ✅ Done | 14 functions in `log_impl.rs` (wraps Rust `log` crate) |
| `unicode` | `char::` | ✅ Done | 9 functions in `unicode_ops.rs` |
| `strconv` | `parse()` methods | ✅ Done | 13 functions in `strconv_ops.rs` |
| `json` | `serde_json` | ⚠️ See below | Already implemented but categorized B |
| `io` | `Read`/`Write` traits | ✅ Done | 2 functions in `io_ops.rs` |
| `time` | System time APIs | ✅ Done | 4 functions in `time_impl.rs` |
| `os` | System calls | ✅ Done | 11 functions in `os_impl.rs` |
| `byte` | `u8` operations | ✅ Done | 4 functions in `byte_ops.rs` |

**Note:** `json` is technically Category B but has already been implemented. We kept it in place since it preserves Go semantics exactly.

**Category A is complete.** All simple utilities with native Rust equivalents are reimplemented and tested.

### Category B: Algorithmic — Transpile via Gourd (②)

Deterministic algorithms. Type-mappable. No runtime magic. Not worth reimplementing.

| Library | Status | Why Transpile? | Notes |
|---------|--------|---------------|-------|
| `json` | ✅ Done | JSON encoding/decoding logic | Preserves Go semantics exactly — already implemented |
| `csv` | ❌ TODO | Row-by-row parsing logic | `csv` crate exists but Go's CSV format nuances matter |
| `math/big` | ❌ TODO | Arbitrary precision arithmetic | No Rust equivalent for Go's `big.Int`/`big.Float` API |
| `math/rand` | ❌ TODO | Random number generation | Deterministic seeding matters for reproducibility |
| `text/tabwriter` | ❌ TODO | Table formatting logic | Small, deterministic, algorithmic |
| `text/template` | ❌ TODO | Template rendering logic | Parsing engine is algorithmic (but complex) |
| `url` | ❌ TODO | URL parsing logic | `url` crate exists but Go's URL handling has nuances |

### Category C: Runtime-Dependent — Bridge via rust2go (③)

Goroutine coordination. GC interaction. Complex runtime behavior.

| Library | Why Bridge? | Notes |
|---------|-------------|-------|
| `sync` | **Paradigm** — entire concurrency model | Map to Rust ecosystem: `crossbeam`, `rayon`, `tokio`. **Must be transpiled into Rust.** |
| `net` | TCP/UDP stack, DNS resolution, I/O multiplexing | Network stack is runtime-dependent |
| `os` | File descriptors, process spawning, signals | Individual calls bridge piecemeal |
| `net/http` | Request/response lifecycle, connection pooling | HTTP is operations, bridge piecemeal |
| `database/sql` | Connection pooling, row iteration, prepared statements | SQL is operations, bridge piecemeal |
| `image/*` | Image processing, pixel formats | Complex image formats |
| `crypto/*` | Hardware acceleration, streaming APIs | Streaming crypto is runtime-dependent |
| `reflect` | Runtime introspection | Can't be transpiled |
| `unsafe` | Memory manipulation | By definition, runtime-dependent |
| `time` | `time.Time`, timezone handling, monotonic clocks | Complex internal state |

### Category D: Edge Cases — Bridge via rust2go (③)

Experimental, platform-specific, or too complex to bridge individually.

| Library | Why Bridge? | Notes |
|---------|-------------|-------|
| `vendor/golang.org` | Internal packages | Not meant for direct use |
| `vet` | Analysis tool | Not a runtime library |
| `weak` | Experimental feature | Not production-ready |

---

## Visual Summary

```
                ┌─────────────────────────────────────┐
                │     Go stdlib library                │
                └──────────────────┬──────────────────┘
                                   │
                    ┌──────────────▼──────────────┐
                    │ Native Rust equivalent?       │
                    └──────┬───────────────────────┘
                           │
              ┌────────────┴────────────┐
              │ Yes                     │ No
              │                       │
    ┌─────────▼─────────┐      ┌──────▼───────┐
    │ < 500 lines?      │      │ Algorithmic?   │
    └─────┬─────────────┘      └──┬────────────┘
          │ Yes                    │ Yes
  ┌───────▼───────┐        ┌──────▼──────┐
  │ ① Reimplement  │        │ ② Transpile  │
  │  in Rust       │        │  via Gourd   │
  └───────────────┘        └──────┬──────┘
          │ No                    │ No
  ┌───────▼───────┐        ┌──────▼──────┐
  │ ② Transpile    │        │ ③ Bridge     │
  │  via Gourd     │        │  via rust2go │
  └───────────────┘        └─────────────┘
```

---

## How rust2go Works (Bridge Architecture)

rust2go generates Go bindings from Rust interface declarations. You define structs and trait methods in Rust, and the `rust2go-cli` tool generates Go code that handles the FFI layer.

### The Pattern

```rust
// In Rust: user.rs
#[derive(rust2go::R2G, Clone)]
pub struct DemoUser {
    pub name: String,
    pub age: u8,
}

#[rust2go::g2r]
pub trait G2RCall {
    fn demo_log(name: String, age: u8);
    fn demo_convert_name(user: DemoUser) -> String;
}
```

rust2go-cli generates Go code that handles the FFI. Key mechanisms:

| Mechanism | How It Works |
|-----------|-------------|
| **String** | Passed as `C.StringRef` = `{ptr, len}` — pointer to Rust's buffer |
| **Vec\<T\>** | Passed as `C.ListRef` = `{ptr, len}` — pointer to Rust's buffer |
| **Complex types** | Serialized into a buffer, pointer passed to Go |
| **Calling** | `asmcall.CallFuncG0P1()` — assembly-based callback, ~2.3ns |
| **Performance** | ~13x faster than cgo (~2.3ns vs ~29ns per call) |
| **Memory safety** | Requires `GODEBUG=invalidptr=0,cgocheck=0` |

### rust2go Architecture Diagram

```
┌─────────────────────────────────────────────┐
│  Rust side                                  │
│                                             │
│  user.rs: Define types & traits             │
│  ┌─────────────────────────────────────┐    │
│  │ #[derive(R2G)] struct DemoUser {    │    │
│  │     name: String,                   │    │
│  │     age: u8,                        │    │
│  │ }                                   │    │
│  │                                     │    │
│  │ #[rust2go::g2r] trait G2RCall {     │    │
│  │     fn demo_log(name: String);      │    │
│  │ }                                   │    │
│  └─────────────────────────────────────┘    │
│                                             │
│  Generated Rust bindings:                   │
│  - c_G2RCall_demo_log // C function ptr    │
│  - DemoUserRef  // C struct repr           │
│                                             │
│  Generated Go bindings:                     │
│  - DemoUser   // Go struct                 │
│  - G2RCallImpl{}.demo_log()               │
│  - asmcall.CallFuncG0P1()                 │
│                                             │
└────────────────────┬────────────────────────┘
                     │ FFI boundary (C ABI)
                     │ StringRef: {ptr, len}
                     │ ListRef:  {ptr, len}
                     │ Buffer:   serialized data
                     │ ASM callback: ~2.3ns
                     │ CGO callback: ~29ns
                     │
┌────────────────────▼────────────────────────┐
│  Go side                                    │
│                                             │
│  impl.go: Implement the interface           │
│  ┌─────────────────────────────────────┐    │
│  │ func (Demo) demo_log(name *string)  │    │
│  │     fmt.Printf("name: %s", name)    │    │
│  │ }                                   │    │
│  └─────────────────────────────────────┘    │
└─────────────────────────────────────────────┘
```

---

## The Bridging Rubric Checklist

### What CAN Be Bridged Through FFI

These are straightforward — they're just data passing through a C ABI:

| Bridgable | How |
|-----------|-----|
| **Primitives** | int, bool, float, pointers |
| **Flat structs** | `#[derive(R2G)]` with simple fields |
| **String** | `ptr + len` pair |
| **Vec\<T\>** | Array reference (pointer + length) |
| **Maps** | Key-value pairs in a buffer |
| **Functions** | FFI function pointers |
| **Lock-free queues** | Shared memory mmap (rust2go's mem-ring) |
| **Callbacks** | Rust calls Go, Go calls back Rust |

### What CANNOT Be Bridged (Fundamentally)

These are barriers caused by Go's runtime model:

| Not Bridgeable | Why |
|---------------|-----|
| **Go's GC and Rust's memory** | Go's GC doesn't see Rust's heap. Rust's allocator doesn't see Go's heap. Cross-boundary ownership is impossible. |
| **Go's goroutine stack** | Goroutines have metadata, scheduler links, stack pointers. Cannot pass through FFI. |
| **Go's interface{} (any type)** | Go's `interface{}` is a double-pointer: (type, value). Rust has no equivalent. |
| **Go's reflection** | Reflection is fundamentally tied to Go's type system. Cannot serialize/deserialize Go struct layouts across FFI. |
| **Go's channels** | Channels are not plain data — they're runtime objects with kernel-side state, scheduler links. |
| **Go's reflection on custom types** | Go's `unsafe.Pointer` on custom structs is fragile across FFI. |

**Rule of thumb**: Cross-boundary ownership is impossible. Always create and destroy on the same side.

### Streaming and Large Data

| Data Size | Bridgeable? | Method |
|-----------|-------------|--------|
| **Small (< 10 MB)** | ✅ Yes | Copy through rust2go buffer |
| **Large (10–100 MB)** | ⚠️ Possible but painful | Multiple allocations, memory pressure |
| **Large (> 100 MB)** | ❌ No | Use cgo for streaming |
| **Streaming bodies** | ❌ No | Must read all into memory first |

---

## Bridging Go's `net/http` via rust2go

The challenge: you **cannot** pass Go's `http.Request` and `http.Response` through FFI. They're complex Go structs with interfaces, maps, slices, pointers, etc.

### The Approach: Simplified HTTP Types

You define a simplified C representation:

```rust
// In Rust: user.rs

#[derive(rust2go::R2G, Clone)]
pub struct HttpHeaders {
    pub keys: Vec<String>,
    pub values: Vec<String>,
}

#[derive(rust2go::R2G, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: HttpHeaders,
    pub body: Vec<u8>,
}

#[derive(rust2go::R2G, Clone)]
pub struct HttpResponse {
    pub status_code: i32,
    pub headers: HttpHeaders,
    pub body: Vec<u8>,
}

#[rust2go::g2r]
pub trait HttpCall {
    fn http_get(url: String) -> HttpResponse;
    fn http_post(url: String, body: Vec<u8>) -> HttpResponse;
    fn http_request(req: HttpRequest) -> HttpResponse;
}
```

### The Go Bridge Layer

rust2go generates the plumbing. You write thin wrapper functions:

```go
// In Go: http_bridge.go

func go_http_get(url string) HttpResponse {
    // Convert Go's http.Request → simplified representation
    req, _ := http.NewRequest("GET", url, nil)
    resp, _ := http.DefaultClient.Do(req)
    defer resp.Body.Close()
    
    // Convert Go's http.Response → simplified representation
    body, _ := io.ReadAll(resp.Body)
    return HttpResponse{
        Status_Code: int32(resp.StatusCode),
        Headers: HttpHeaders{
            Keys:   resp.Header.Values(),
            Values: resp.Header.Values(),
        },
        Body: body,
    }
}
```

### What This Actually Gives You

| What | How It Works |
|------|-------------|
| **Go stdlib stays in Go** | `http.Get`, `http.Post`, etc. run in Go's runtime |
| **Transpiled code gets HTTP** | Rust calls Go's HTTP through rust2go bridge |
| **Type safety** | Structs are mapped: `String` ↔ C string, `Vec<T>` ↔ C array |
| **Performance** | ~2.3ns per call (asmcall) vs ~29ns (cgo) |
| **Memory** | Minimal copies — Rust passes buffers directly |

### The Hard Parts for HTTP

#### Headers — Doable
```rust
// Rust type: Vec<(String, String)>
// C representation: Two parallel Vec<String>
// Go bridge: Flatten http.Header → Vec<String>
```

#### Body — Simple for Small Bodies
```rust
// Read body into Vec<u8> on the Go side
body, _ := io.ReadAll(resp.Body)
// Pass Vec<u8> through FFI as C.ListRef
```

#### Streaming Bodies — Problematic
```
Go side (http.Client)                      Rust side (async)
┌───────────────────┐                   ┌───────────────────┐
│                   │                   │                   │
│  http.Response    │                   │  HttpResponse     │
│  .Body = Reader   │                   │  body: Vec<u8>    │
│                   │                   │                   │
│  ┌─────────────┐  │    FFI boundary   │  ┌─────────────┐  │
│  │  stream     │  │  ← can't cross    │  │  Vec<u8>    │  │
│  │             │  │     the boundary  │  │  (owned)    │  │
│  └─────────────┘  │                   │  └─────────────┘  │
│                   │                   │                   │
└───────────────────┘                   └───────────────────┘
```
- HTTP response body is a streaming reader — reads from a socket
- FFI boundaries are synchronous — the function returns when the call is done
- Rust needs owned data — can't have a Rust struct with a Go pointer to a stream

#### Redirects — Missing
Go's `http.Client` has redirect policies. You'd need to decide:
- Follow redirects automatically (simpler)
- Expose redirect policy to Rust (complex)

#### TLS/Connections — Hidden
Go's `http.Client` handles TLS handshakes, connection pooling, etc. These are transparent — the Go stdlib handles them. The transpiled Rust code just makes an API call.

### Could We Extend rust2go for Streaming?

Three approaches, all problematic:

**Approach 1: Persistent connection handle**
```rust
let handle = HttpCallImpl.http_open("https://...");
while let Some(chunk) = HttpCallImpl.http_read(handle) {
    // process chunk
}
HttpCallImpl.http_close(handle);
```
- **Problem**: Connection state lives on Go side. Rust just has an opaque handle.
- **Risk**: If Rust drops the handle without closing, Go leaks the connection.

**Approach 2: Ring buffer via mmap**
- **Problem**: Need to know when data is "done" (HTTP chunked encoding?)
- **Problem**: Backpressure (what if Rust reads slower than Go writes?)

**Approach 3: Callback-based streaming**
```rust
fn http_callback(chunk: Vec<u8>, done: bool) {
    if done { drop() } else { process(chunk) }
}
HttpCallImpl.http_stream(url, http_callback);
```
- **Problem**: Callbacks are synchronous. Rust must process each chunk before Go writes the next.

**Recommendation**: Don't extend rust2go for streaming. Use cgo directly for streaming cases.

---

## CGO vs rust2go for HTTP

| | rust2go | CGO (manual) |
|--|---------|-------------|
| **Setup** | Generated code, derive macros | Hand-written, error-prone |
| **Streaming** | ❌ No built-in support | ✅ Possible (but complex) |
| **Performance** | ~2.3ns (asmcall) | ~29ns (cgo) |
| **Memory safety** | Better (managed buffers) | Worse (raw pointers) |
| **Complex types** | Generated conversions | Must write manually |
| **Streaming bodies** | Would need extensive work | Just pass a FILE* or go 1.22+ net/http's streaming APIs |

### When to Use Each

| Use rust2go when... | Use cgo when... |
|---------------------|-----------------|
| The HTTP call is request-response | The response body is streaming/large |
| The body fits in memory (< 10 MB) | You need to stream data through FFI |
| You want type safety and code generation | You need full control over the FFI |
| The codebase uses rust2go elsewhere | The bridge is a one-off utility |

---

## HTTP Bridging — Complete Rubric

| Library | Approach | Implementation |
|---------|----------|---------------|
| **`strings`** | ① Reimplement | Rust's `str::` and `String::` |
| **`bytes`** | ① Reimplement | Rust's byte slices |
| **`math`** | ① Reimplement | Rust's native numeric types |
| **`json`** | ② Transpile | `serde_json` exists but Go semantics matter |
| **`time`** | ③ Bridge | Go's time handling is complex |
| **`os`** | ③ Bridge | Individual calls bridge piecemeal |
| **`database/sql`** | ③ Bridge | SQL is operations, bridge piecemeal |
| **`net/http`** | ③ Bridge (small) / ④ CGO (large) | rust2go for request-response, cgo for streaming |
| **`sync`** | Transpile to Rust ecosystem | Must be transpiled into Rust concurrency model |

---

## How It Applies to Your Use Case

### When to Transpile the User's Go Code (Gourd)

```rust
// User's Go code → transpile to Rust
func Process() string {
    // Algorithmic logic → transpile
    result := strings.ToUpper(data)      // → Rust code
    result := strings.ReplaceAll(result, " ", "-")  // → Rust code
    result := strings.Trim(result, " \n")  // → Rust code
    return result
}
```

### When to Reimplement stdlib (Rust Native)

Instead of transpiling or bridging:

| Go | Rust |
|----|------|
| `strings.ReplaceAll(s, " ", "-")` | `s.replace(" ", "-")` |
| `strings.ToUpper(s)` | `s.to_uppercase()` |
| `sort.Slice(data, func(i, j int) bool { ... })` | `data.sort_by(|a, b| a.cmp(b))` |

### When to Bridge Individual Calls (rust2go)

Instead of reimplementing or transpiling:

| Go | Rust (via rust2go bridge) |
|----|--------------------------|
| `http.Get("https://...")` | `go_http_get("https://...")` |
| `db.Query("SELECT * FROM users")` | `go_db_query("SELECT * FROM users")` |
| `os.ReadFile("config.json")` | `go_os_read_file("config.json")` |

### When to Transpile the Concurrency Model (Gourd → Rust Ecosystem)

Go's `sync` library is the only stdlib library that is a **paradigm**, not a collection of operations. You can't bridge goroutines through FFI — the scheduler is fundamentally Go's. Transpile the whole concurrency model into Rust:

| Go (`sync`) | Rust (ecosystem) |
|-------------|-----------------|
| `go func() { ... }` | `crossbeam::thread::spawn()` or `rayon::spawn()` |
| `make(chan T)` | `crossbeam::channel::bounded()` or `flume::bounded()` |
| `sync.Mutex` | `crossbeam::lock::Mutex` or `std::sync::Mutex` |
| `sync.WaitGroup` | `crossbeam::thread::scoped()` |
| `sync.RWMutex` | `crossbeam::lock::RwLock` |
| `select { case ... default ... }` | `crossbeam::select!()` or `tokio::select!` |
| `sync.Once` | `once_cell::sync::OnceCell` |
| `sync.Pool` | Custom implementation or `moka::sync::Pool` |

---

## Migration Strategy Summary

```
Phase 1: Everything bridges through rust2go
    │
    ├─ User replaces stdlib calls with Rust equivalents
    │   (strings → str::, json → serde_json, etc.)
    │
    ├─ User replaces sync → Rust concurrency model
    │   (goroutines → crossbeam/rayon, channels → crossbeam)
    │
    └─ Eventually: all stdlib calls replaced
                    all bridge calls removed
                    100% Rust
```
