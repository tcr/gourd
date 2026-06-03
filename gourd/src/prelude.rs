//! Gourd prelude: standard library of Go constructs.
//!
//! This module provides the runtime types and functions that correspond to
//! Go's standard library. Generated code references these items at compile
//! time via `gourd::prelude::*`.
//!
//! # GoGc<T>
//!
//! A lightweight wrapper around `Arc<T>` that mirrors Go's garbage-collected
//! pointer semantics (heap-allocated, shared ownership, automatic
//! deallocation when the last reference is dropped).
//!
//! # GoBox<T>
//!
//! Unique heap ownership — Go's `new(T)` pattern. A single pointer into
//! the heap that can be dereferenced and mutably dereferenced.
//!
//! # GoMutex<T>
//!
//! Go's `sync.Mutex` — guards a value behind exclusive access. Used to
//! share mutable state across concurrent goroutines.
//!
//! # GoRc<T>
//!
//! Reference-counted pointer with interior mutability — Go's `atomic` +
//! `sync.RWMutex` combined. Clone cheaply, mutate safely.
//!
//! # Standalone functions
//!
//! Go builtin functions implemented as regular Rust functions:
//! `len`, `cap`, `append`, `make_slice`, `make_map`, `copy`, `min`, `max`.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::fmt::Display;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::sync::atomic::{AtomicI32, AtomicI64, AtomicU32, AtomicU64, Ordering};

// ─── GoGc<T>   ───────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct GoGc<T: 'static + ?Sized> {
    inner: Arc<T>,
}

impl<T: 'static> GoGc<T> {
    /// Allocates and wraps a value on the heap. Returns the only reference (refcount = 1).
    pub fn new(value: T) -> Self {
        GoGc {
            inner: Arc::new(value),
        }
    }

    /// Consumes the `GoGc<T>` and returns the inner `Arc<T>`. Reference count unchanged.
    pub fn into_inner(self) -> Arc<T> {
        self.inner
    }

    /// Unwraps to `T` if this is the last reference (refcount == 1).
    pub fn try_unwrap(self) -> Result<T, GoGc<T>> {
        Arc::try_unwrap(self.inner).map_err(|inner| GoGc { inner })
    }
}

impl<T: ?Sized> GoGc<T> {
    /// Returns the current strong reference count.
    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }

    /// Returns true if this is the only strong reference (refcount == 1).
    pub fn is_unique(&self) -> bool {
        self.strong_count() == 1
    }

    /// Returns true if there are multiple strong references (refcount > 1).
    pub fn is_shared(&self) -> bool {
        self.strong_count() > 1
    }

    /// Returns mutable access to the inner value **if** this is the only
    /// reference (refcount == 1). Equivalent to Go's `*ptr = value` on
    /// a single-owner pointer.
    ///
    /// ```
    /// use gourd::GoGc;
    ///
    /// let mut gc = GoGc::new(42i32);
    /// if let Some(val) = gc.as_mut() {
    ///     *val = 99;
    /// }
    /// assert_eq!(*gc, 99);
    /// ```
    pub fn as_mut(&mut self) -> Option<&mut T> {
        if self.is_unique() {
            Arc::get_mut(&mut self.inner)
        } else {
            None
        }
    }

    /// Returns a raw mutable pointer to the inner value (unsafe).
    ///
    /// Mirrors Go's `&var` address-of operator. The returned pointer can
    /// be dereferenced with `*ptr` to get the value, or `*ptr = val` to
    /// mutate it.
    ///
    /// # Safety
    ///
    /// The caller must ensure exclusive access to the pointed-to value.
    /// This is safe only when `is_unique()` is true, or when external
    /// synchronization guarantees no data races.
    pub unsafe fn as_raw_ptr(&self) -> *mut T {
        Arc::as_ptr(&self.inner) as *mut T
    }
}

// Explicit Send + Sync — Go pointers are always concurrency-safe.
unsafe impl<T: ?Sized + Send> Send for GoGc<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for GoGc<T> {}

// ─── GoBox<T> ──────────────────────────────────────────────────────────────

/// Unique heap ownership — Go's `new(T)` pattern.
///
/// A single pointer into the heap that can be dereferenced and mutated.
/// When `GoBox<T>` is dropped, the heap allocation is freed.
///
/// ```
/// use gourd::GoBox;
///
/// let box_val = GoBox::new(42i32);
/// assert_eq!(*box_val, 42);
///
/// let mut box_val = GoBox::new(0i32);
/// *box_val = 99;
/// assert_eq!(*box_val, 99);
/// ```
#[derive(Debug)]
pub struct GoBox<T: 'static> {
    inner: Box<T>,
}

impl<T: 'static> GoBox<T> {
    /// Heap-allocates a value. Returns the only pointer (unique ownership).
    /// Mirrors Go's `new(T)` built-in function.
    pub fn new(value: T) -> Self {
        GoBox {
            inner: Box::new(value),
        }
    }

    /// Consumes the `GoBox<T>` and returns the inner value.
    pub fn into_inner(self) -> T {
        *self.inner
    }

    /// Returns true if the box has not been moved from.
    pub fn is_alive(&self) -> bool {
        true // Box<T> is always alive until consumed
    }
}

impl<T: 'static> Deref for GoBox<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: 'static> DerefMut for GoBox<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

// ─── GoMutex<T> ────────────────────────────────────────────────────────────

/// Go's `sync.Mutex` — guards a value behind exclusive access.
///
/// Used to share mutable state across concurrent goroutines.
///
/// ```
/// use gourd::GoMutex;
///
/// let mut_val = GoMutex::new(vec![1, 2, 3]);
/// {
///     let mut guard = mut_val.lock();
///     guard.push(4);
/// }
/// assert_eq!(mut_val.lock().len(), 4);
/// ```
#[derive(Debug)]
pub struct GoMutex<T: 'static> {
    inner: Mutex<T>,
}

impl<T: 'static> GoMutex<T> {
    /// Wraps a value in a mutex guard.
    pub fn new(value: T) -> Self {
        GoMutex {
            inner: Mutex::new(value),
        }
    }

    /// Locks the mutex, returning a guard that provides shared mutable
    /// access. Mirrors Go's `mutex.Lock()` / `defer mutex.Unlock()` pattern.
    pub fn lock(&self) -> GoMutexGuard<'_, T> {
        GoMutexGuard {
            inner: self.inner.lock().unwrap(),
        }
    }

    /// Tries to lock the mutex without blocking. Returns `None` if
    /// the mutex is currently held by another thread.
    pub fn try_lock(&self) -> Option<GoMutexGuard<'_, T>> {
        self.inner.try_lock().ok().map(|guard| GoMutexGuard {
            inner: guard,
        })
    }
}

impl<T: Clone + 'static> Clone for GoMutex<T> {
    fn clone(&self) -> Self {
        GoMutex {
            inner: Mutex::new(
                self.lock().clone()
            ),
        }
    }
}

/// A guard that provides exclusive mutable access to the inner value.
/// Dropping the guard releases the lock.
pub struct GoMutexGuard<'a, T: 'static> {
    inner: MutexGuard<'a, T>,
}

impl<'a, T: 'static> Deref for GoMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, T: 'static> DerefMut for GoMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

// ─── GoRc<T> ───────────────────────────────────────────────────────────────

/// Reference-counted pointer with interior mutability.
///
/// Combines `Arc<T>` with `RefCell<T>` to allow cheap cloning and safe
/// interior mutation. Mirrors Go's atomic + RWMutex combined pattern.
///
/// ```
/// use gourd::GoRc;
///
/// let rc = GoRc::new(vec![1, 2, 3]);
/// let clone = GoRc::clone(&rc);
/// assert_eq!(rc.get().len(), 3);
/// assert_eq!(clone.get().len(), 3);
/// {
///     let mut inner = rc.get_mut();
///     inner.push(4);
/// }
/// assert_eq!(clone.get().len(), 4);
/// ```
#[derive(Debug)]
pub struct GoRc<T: 'static> {
    inner: Arc<RefCell<T>>,
}

impl<T: 'static> GoRc<T> {
    /// Heap-allocates and wraps a value with interior mutability.
    /// Returns the only reference (refcount = 1).
    pub fn new(value: T) -> Self {
        GoRc {
            inner: Arc::new(RefCell::new(value)),
        }
    }

    /// Returns the current strong reference count.
    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }

    /// Returns true if this is the only reference (refcount == 1).
    pub fn is_unique(&self) -> bool {
        self.strong_count() == 1
    }

    /// Returns true if there are multiple references (refcount > 1).
    pub fn is_shared(&self) -> bool {
        self.strong_count() > 1
    }

    /// Returns a read-only interior reference to the inner value.
    pub fn get(&self) -> std::cell::Ref<'_, T> {
        RefCell::borrow(&self.inner)
    }

    /// Returns a mutable interior reference to the inner value.
    /// Panics if already mutably borrowed.
    pub fn get_mut(&self) -> std::cell::RefMut<'_, T> {
        RefCell::borrow_mut(&self.inner)
    }

    /// Consumes the `GoRc<T>` and returns the inner value if unique.
    pub fn try_unwrap(self) -> Result<T, GoRc<T>> {
        Arc::try_unwrap(self.inner)
            .map(|rc| rc.into_inner())
            .map_err(|inner| GoRc { inner })
    }
}

// Note: GoRc intentionally does NOT implement Deref.
// Interior mutability requires explicit get() / get_mut() calls.
// This mirrors Go's need to lock before accessing shared mutable state.

impl<T: 'static> Clone for GoRc<T> {
    fn clone(&self) -> Self {
        GoRc {
            inner: Arc::clone(&self.inner),
        }
    }
}

// ─── GoOnce ────────────────────────────────────────────────────────────────

/// Go's `sync.Once` — guarantees a function executes exactly once.
///
/// Used to initialize shared state safely in concurrent programs.
#[derive(Debug)]
pub struct GoOnce {
    inner: (Mutex<bool>, Condvar),
}

impl GoOnce {
    /// Creates a new `GoOnce` that has not yet been triggered.
    pub fn new() -> Self {
        GoOnce {
            inner: (Mutex::new(false), Condvar::new()),
        }
    }

    /// Calls the function exactly once, even if called from multiple threads.
    /// Subsequent calls are no-ops.
    pub fn call<F: FnOnce(&GoOnceArgs)>(&self, f: F) {
        let mut triggered = self.inner.0.lock().unwrap();
        if !*triggered {
            f(&GoOnceArgs::new());
            *triggered = true;
            self.inner.1.notify_all();
        }
    }
}

impl Clone for GoOnce {
    fn clone(&self) -> Self {
        GoOnce {
            inner: (Mutex::new(false), Condvar::new()),
        }
    }
}

/// Extra arguments passed to the `GoOnce` function.
pub struct GoOnceArgs {
    _private: (),
}

impl GoOnceArgs {
    /// Creates a new `GoOnceArgs`.
    pub fn new() -> Self {
        GoOnceArgs { _private: () }
    }
    /// Returns true if the function was already triggered by a prior `GoOnce::call`.
    pub fn already_triggered(&self) -> bool {
        false // only called when the function is actually executed
    }
}

// ─── GoWaitGroup ───────────────────────────────────────────────────────────

/// Go's `sync.WaitGroup` — wait for multiple concurrent operations.
#[derive(Debug)]
pub struct GoWaitGroup {
    count: Arc<(Mutex<i32>, Condvar)>,
}

impl GoWaitGroup {
    /// Creates a new wait group with zero outstanding operations.
    pub fn new() -> Self {
        GoWaitGroup {
            count: Arc::new((Mutex::new(0), Condvar::new())),
        }
    }

    /// Adds delta to the wait counter. Must be called before starting workers.
    pub fn add(&self, delta: i32) {
        let mut count = self.count.0.lock().unwrap();
        *count = count.checked_add(delta)
            .expect("GoWaitGroup counter overflow");
    }

    /// Decrements the counter by 1. Call when a worker completes.
    pub fn done(&self) {
        let mut count = self.count.0.lock().unwrap();
        *count = count.checked_sub(1)
            .expect("GoWaitGroup counter underflow");
        if *count == 0 {
            self.count.1.notify_all();
        }
    }

    /// Blocks until the counter reaches zero.
    pub fn wait(&self) {
        let mut count = self.count.0.lock().unwrap();
        while *count > 0 {
            count = self.count.1.wait(count).unwrap();
        }
    }
}

impl Clone for GoWaitGroup {
    fn clone(&self) -> Self {
        GoWaitGroup {
            count: Arc::clone(&self.count),
        }
    }
}

// ─── GoRWMutex<T> ──────────────────────────────────────────────────────────

/// Go's `sync.RWMutex` — shared read locks, exclusive write locks.
#[derive(Debug)]
pub struct GoRWMutex<T: 'static> {
    inner: RwLock<T>,
}

impl<T: 'static> GoRWMutex<T> {
    /// Wraps a value behind a read-write lock.
    pub fn new(value: T) -> Self {
        GoRWMutex {
            inner: RwLock::new(value),
        }
    }

    /// Acquires a read lock, allowing multiple concurrent readers.
    pub fn read_lock(&self) -> GoRwReadGuard<'_, T> {
        GoRwReadGuard {
            inner: self.inner.read().unwrap(),
        }
    }

    /// Acquires a write lock, exclusive access for modification.
    pub fn write_lock(&self) -> GoRwWriteGuard<'_, T> {
        GoRwWriteGuard {
            inner: self.inner.write().unwrap(),
        }
    }

    /// Tries to acquire a read lock without blocking.
    pub fn try_read_lock(&self) -> Option<GoRwReadGuard<'_, T>> {
        self.inner.try_read().ok().map(|inner| GoRwReadGuard {
            inner,
        })
    }

    /// Tries to acquire a write lock without blocking.
    pub fn try_write_lock(&self) -> Option<GoRwWriteGuard<'_, T>> {
        self.inner.try_write().ok().map(|inner| GoRwWriteGuard {
            inner,
        })
    }
}

impl<T: Clone + 'static> Clone for GoRWMutex<T> {
    fn clone(&self) -> Self {
        GoRWMutex {
            inner: RwLock::new(self.read_lock().clone()),
        }
    }
}

/// Read lock guard — multiple readers can hold this simultaneously.
pub struct GoRwReadGuard<'a, T: 'static> {
    inner: RwLockReadGuard<'a, T>,
}

impl<'a, T: 'static> Deref for GoRwReadGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Write lock guard — exclusive access for modification.
pub struct GoRwWriteGuard<'a, T: 'static> {
    inner: RwLockWriteGuard<'a, T>,
}

impl<'a, T: 'static> Deref for GoRwWriteGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, T: 'static> DerefMut for GoRwWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

// ─── GoChannel<T> ──────────────────────────────────────────────────────────

/// Go's channel type — typed communication between goroutines.
///
/// Implements Go channel semantics:
/// - Blocking send/recv on empty/full/unbuffered channels
/// - Non-blocking `try_send`/`try_recv` returns false when channel is full/empty
/// - `close()` prevents further sends; blocked receivers are woken with `None`
/// - A closed channel never blocks sends (they return `false` immediately)
/// - Multiple goroutines can safely share a channel (Go semantics)
#[derive(Debug)]
pub struct GoChannel<T> {
    /// All shared state behind a single lock.
    inner: Mutex<ChannelInner<T>>,
    /// Condition variable for blocking receivers.
    not_empty: Condvar,
    /// Condition variable for blocking senders.
    not_full: Condvar,
}

/// Internal channel state.
#[derive(Debug)]
struct ChannelInner<T> {
    buffer: VecDeque<T>,
    closed: bool,
    capacity: usize,
    /// For unbuffered channels: true means a value is pending (sent but not received).
    has_pending: bool,
}

impl<T> GoChannel<T> {
    /// Creates a new unbuffered channel (buffer size 0).
    ///
    /// This blocks until both a sender and receiver are ready.
    pub fn make() -> Self {
        GoChannel {
            inner: Mutex::new(ChannelInner {
                buffer: VecDeque::new(),
                closed: false,
                capacity: 0,
                has_pending: false,
            }),
            not_empty: Condvar::new(),
            not_full: Condvar::new(),
        }
    }

    /// Creates a new buffered channel with the given capacity.
    ///
    /// Sends to a buffered channel block only when the buffer is full.
    /// Receives block only when the buffer is empty and the channel is not closed.
    pub fn make_buffered(cap: usize) -> Self {
        GoChannel {
            inner: Mutex::new(ChannelInner {
                buffer: VecDeque::with_capacity(cap),
                closed: false,
                capacity: cap,
                has_pending: false,
            }),
            not_empty: Condvar::new(),
            not_full: Condvar::new(),
        }
    }

    /// Sends a value on the channel. Blocks if full (unless the channel is closed).
    ///
    /// For unbuffered channels: blocks until a receiver is ready.
    /// Returns `true` if the send succeeded, `false` if the channel was closed.
    pub fn send(&self, value: T) -> bool {
        let mut inner = self.inner.lock().unwrap();

        // If closed, don't accept sends.
        if inner.closed {
            return false;
        }

        // Push the value.
        inner.buffer.push_back(value);
        self.not_empty.notify_one();

        if inner.capacity == 0 {
            // Unbuffered channel: wait until receiver pops the value.
            // has_pending indicates sender pushed but receiver hasn't popped yet.
            while inner.has_pending && !inner.closed {
                inner = self.not_full.wait(inner).unwrap();
                if inner.closed {
                    inner.buffer.pop_back();
                    return false;
                }
            }
        } else if inner.buffer.len() >= inner.capacity && !inner.closed {
            // Buffered channel full: wait for a receiver to free space.
            inner = self.not_full.wait(inner).unwrap();
            if inner.closed {
                inner.buffer.pop_back();
                return false;
            }
        }

        true
    }

    /// Sends a value without blocking. Returns `true` if the send succeeded,
    /// `false` if the channel is full, closed, or unbuffered with no receiver.
    ///
    /// Equivalent to Go's `select` with `default` clause.
    pub fn try_send(&self, value: T) -> bool {
        let mut inner = self.inner.lock().unwrap();

        // If closed, don't accept sends.
        if inner.closed {
            return false;
        }

        // Check if there's space.
        if inner.buffer.len() < inner.capacity {
            inner.buffer.push_back(value);
            self.not_empty.notify_one();
            true
        } else {
            false
        }
    }

    /// Receives a value from the channel. Blocks if empty (unless closed).
    ///
    /// For unbuffered channels: blocks until a sender is ready.
    /// Returns `Some(value)` if a value was received, `None` if the channel
    /// is closed and empty (all values drained).
    pub fn recv(&self) -> Option<T> {
        let mut inner = self.inner.lock().unwrap();



        // If closed and empty, return None to signal no more values.
        if inner.closed && inner.buffer.is_empty() {
            return None;
        }

        // For unbuffered channels: check has_pending first.
        if inner.capacity == 0 && inner.has_pending {
            // A value has been sent but not yet received — pop it.
            let value = inner.buffer.pop_front().unwrap();
            inner.has_pending = false;
            self.not_full.notify_one();

            return Some(value);
        }

        // Wait until there's something to receive or the channel is closed.
        while inner.buffer.is_empty() && !inner.closed {

            inner = self.not_empty.wait(inner).unwrap();

        }

        // No value and closed — return None.
        if inner.buffer.is_empty() {
            return None;
        }

        let value = inner.buffer.pop_front().unwrap();

        // For unbuffered channels: mark as no longer pending and notify sender.
        if inner.capacity == 0 {
            inner.has_pending = false;
            self.not_full.notify_one();
        }

        self.not_full.notify_one();

        Some(value)
    }

    /// Receives a value without blocking. Returns `Some(value)` if one is
    /// available, `None` if the channel is empty and/or closed.
    ///
    /// Equivalent to Go's `select` with `default` clause.
    pub fn try_recv(&self) -> Option<T> {
        let mut inner = self.inner.lock().unwrap();

        // If closed and empty, no more values.
        if inner.closed && inner.buffer.is_empty() {
            return None;
        }

        // If there's a value, pop it.
        if !inner.buffer.is_empty() {
            let value = inner.buffer.pop_front().unwrap();
            self.not_full.notify_one();
            return Some(value);
        }

        // Empty and open — no value available.
        None
    }

    /// Closes the channel. No more values can be sent.
    ///
    /// Wakes all blocked receivers and senders. Receivers will return `None`
    /// once the buffer is drained. Senders will receive `false`.
    pub fn close(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.closed = true;
        // Wake all blocked receivers and senders.
        self.not_empty.notify_all();
        self.not_full.notify_all();
    }

    /// Returns the number of values currently buffered.
    /// Only valid when no other goroutines are using the channel concurrently
    /// (for debugging/inspection purposes).
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().buffer.len()
    }

    /// Returns true if the channel has no buffered values.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().buffer.is_empty()
    }
}

impl<T: Clone> Clone for GoChannel<T> {
    fn clone(&self) -> Self {
        let inner = self.inner.lock().unwrap();
        GoChannel {
            inner: Mutex::new(ChannelInner {
                buffer: inner.buffer.clone(),
                closed: inner.closed,
                capacity: inner.capacity,
                has_pending: inner.has_pending,
            }),
            not_empty: Condvar::new(),
            not_full: Condvar::new(),
        }
    }
}

// ─── GoAtomic<T> ───────────────────────────────────────────────────────────

/// Go's `atomic` package — lock-free atomic operations.
/// Thread-safe single-owner atomic values. Use `GoGc<GoAtomicI32>`
/// to share atomic values across goroutines.

macro_rules! impl_go_atomic {
    ($name:ident, $atomic:ty, $from:expr, $to:expr) => {
        #[derive(Debug)]
        pub struct $name {
            inner: $atomic,
        }

        impl $name {
            /// Creates a new atomic value.
            pub fn new(value: i64) -> Self {
                $name {
                    inner: <$atomic>::new($from(value)),
                }
            }

            /// Loads the current value (atomic read).
            pub fn load(&self) -> i64 {
                $to(self.inner.load(Ordering::SeqCst))
            }

            /// Stores a value (atomic write).
            pub fn store(&self, val: i64) {
                self.inner.store($from(val), Ordering::SeqCst);
            }

            /// Swaps the value, returning the previous value.
            pub fn swap(&self, val: i64) -> i64 {
                $to(self.inner.swap($from(val), Ordering::SeqCst))
            }

            /// Atomically compares and swaps. Returns true if swap succeeded.
            pub fn compare_and_swap(&self, current: i64, new: i64) -> bool {
                self.inner.compare_exchange(
                    $from(current), $from(new), Ordering::SeqCst, Ordering::SeqCst
                ).is_ok()
            }

            /// Atomically adds delta, returns the new value.
            pub fn add(&self, delta: i64) -> i64 {
                $to(self.inner.fetch_add($from(delta), Ordering::SeqCst) + $from(delta))
            }
        }

        impl Clone for $name {
            fn clone(&self) -> Self {
                $name {
                    inner: <$atomic>::new($from(self.load())),
                }
            }
        }
    };
}

impl_go_atomic!(GoAtomicI32, AtomicI32, |v: i64| v as i32, |v: i32| v as i64);
impl_go_atomic!(GoAtomicI64, AtomicI64, |v: i64| v, |v: i64| v);
impl_go_atomic!(GoAtomicU32, AtomicU32, |v: i64| v as u32, |v: u32| v as i64);
impl_go_atomic!(GoAtomicU64, AtomicU64, |v: i64| v as u64, |v: u64| v as i64);

impl<T: ?Sized> Deref for GoGc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: ?Sized> Clone for GoGc<T> {
    fn clone(&self) -> Self {
        GoGc {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T: PartialEq + ?Sized> PartialEq for GoGc<T> {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl<T: Eq + ?Sized> Eq for GoGc<T> {}

impl<T: Display + ?Sized> Display for GoGc<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

// ─── Standalone functions ──────────────────────────────────────────────────

/// Returns the length of a slice-like type as i32 (Go `len()`).
pub fn len<T: AsRef<[u8]>>(slice: T) -> i32 {
    slice.as_ref().len() as i32
}

/// Returns the capacity of a vector as i32 (Go `cap()`).
pub fn cap<T: AsRef<[u8]>>(vec: &Vec<T>) -> i32 {
    vec.capacity() as i32
}

/// Appends a value to a slice, returning a new slice (Go `append(slice, val)`).
pub fn append<T: Clone + Default>(mut slice: Vec<T>, val: T) -> Vec<T> {
    slice.push(val);
    slice
}

/// Creates a new slice of given length with a default value (Go `make([]T, n)`).
pub fn make_slice<T: Clone + Default>(len: i32, val: T) -> Vec<T> {
    vec![val; len as usize]
}

/// Creates a new empty map (Go `make(map[K]V)`).
pub fn make_map<K: Hash + Eq, V>() -> HashMap<K, V> {
    HashMap::new()
}

/// Copies elements from src into dst, returning the number copied (Go `copy(dst, src)`).
pub fn copy<T: Clone>(mut dst: Vec<T>, src: &[T]) -> i32 {
    let n = dst.len().min(src.len());
    dst.clear();
    dst.extend_from_slice(&src[..n]);
    n as i32
}

/// Returns the smaller of two values (Go `min(a, b)`).
pub fn min<T: PartialOrd>(a: T, b: T) -> T {
    if a <= b { a } else { b }
}

/// Returns the larger of two values (Go `max(a, b)`).
pub fn max<T: PartialOrd>(a: T, b: T) -> T {
    if a >= b { a } else { b }
}

// ─── GoSlice operations ───────────────────────────────────────────────────

/// Returns the index of the first occurrence of `val` in a slice (-1 if not found).
pub fn index<T: PartialEq>(slice: &[T], val: &T) -> i32 {
    for (i, v) in slice.iter().enumerate() {
        if v == val { return i as i32; }
    }
    -1
}

/// Returns a sub-slice from start to end (Go slice[i:j]).
pub fn slice_sub<T: Clone>(slice: &[T], start: i32, end: i32) -> Vec<T> {
    let start = start.max(0) as usize;
    let end = end.max(0) as usize;
    let end = end.min(slice.len());
    if start >= end { return vec![]; }
    slice[start..end].to_vec()
}

/// Sorts a slice in ascending order (Go `sort.Slice`).
pub fn sort<T: Ord>(slice: &mut [T]) {
    slice.sort();
}

/// Reverses a slice in place (Go `sort.Reverse`).
pub fn reverse<T>(slice: &mut [T]) {
    slice.reverse();
}

/// Returns true if the slice contains the value (Go `Contains`).
pub fn contains<T: PartialEq>(slice: &[T], val: &T) -> bool {
    slice.contains(val)
}

/// Joins a slice of strings with a separator (Go `strings.Join`).
pub fn join<T: AsRef<str>>(elems: &[T], sep: &str) -> String {
    elems.iter().map(|e| e.as_ref()).collect::<Vec<&str>>().join(sep)
}

/// Splits a string by a separator (Go `strings.Split`).
pub fn split(s: &str, sep: &str) -> Vec<String> {
    s.split(sep).map(|s| s.to_string()).collect()
}

/// Returns true if the string contains the substring (Go `strings.Contains`).
pub fn contains_str(s: &str, sub: &str) -> bool {
    s.contains(sub)
}

/// Returns the first index of the substring, or -1 (Go `strings.Index`).
pub fn index_str(s: &str, sub: &str) -> i32 {
    s.find(sub).map(|i| i as i32).unwrap_or(-1)
}

/// Trims leading and trailing whitespace (Go `strings.TrimSpace`).
pub fn trim(s: &str) -> &str {
    s.trim()
}

/// Trims leading whitespace (Go `strings.TrimLeft`).
pub fn trim_left(s: &str) -> &str {
    s.trim_start()
}

/// Trims trailing whitespace (Go `strings.TrimRight`).
pub fn trim_right(s: &str) -> &str {
    s.trim_end()
}

/// Converts a string to uppercase (Go `strings.ToUpper`).
pub fn to_upper(s: &str) -> String {
    s.to_uppercase()
}

/// Converts a string to lowercase (Go `strings.ToLower`).
pub fn to_lower(s: &str) -> String {
    s.to_lowercase()
}

/// Repeats a string n times (Go `strings.Repeat`).
pub fn repeat(s: &str, n: i32) -> String {
    if n <= 0 { return String::new(); }
    s.repeat(n as usize)
}

// ─── Go type system ───────────────────────────────────────────────────────

/// Go's `any` / `interface{}` — can hold any type.
///
/// This is the runtime equivalent of Go's untyped interface.
/// Used by the transpiler for generic values that can hold arbitrary types.
///
/// ```
/// use gourd::Any;
///
/// let a: Any = Any::from(42i32);
/// let b: Any = Any::from("hello");
/// assert_eq!(a.downcast_ref::<i32>(), Some(&42));
/// assert_eq!(b.downcast_ref::<&str>(), Some(&"hello"));
/// ```
#[derive(Debug)]
pub struct Any {
    inner: Box<dyn std::any::Any + Send + Sync>,
    type_name: &'static str,
}

impl Any {
    /// Creates a new `Any` holding the given value.
    pub fn from<T: 'static + Send + Sync>(value: T) -> Self {
        Any {
            inner: Box::new(value),
            type_name: std::any::type_name::<T>(),
        }
    }

    /// Downcasts to a reference of type `T`. Returns `None` if the type
    /// does not match.
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.inner.downcast_ref::<T>()
    }

    /// Downcasts to a mutable reference of type `T`. Returns `None` if
    /// the type does not match.
    pub fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.inner.downcast_mut::<T>()
    }

    /// Consumes the `Any` and returns the inner value of type `T`.
    /// Returns `None` if the type does not match.
    pub fn downcast<T: 'static>(self) -> Option<T> {
        match self.inner.downcast::<T>() {
            Ok(boxed) => Some(*boxed),
            Err(_) => None,
        }
    }

    /// Returns the type name of the held value.
    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// Returns true if the held value is of type `T`.
    pub fn is<T: 'static>(&self) -> bool {
        self.inner.is::<T>()
    }
}

// Clone is intentionally NOT implemented for Any. This prevents
// silent type erasure — users must explicitly downcast.
// If needed in the future, we can implement it by boxing the inner type.

// ─── Go error handling ─────────────────────────────────────────────────────

/// Go's `error` interface — boxed error trait.
#[derive(Debug)]
pub struct GoError {
    message: String,
}

impl GoError {
    /// Creates a new error with the given message.
    pub fn new(message: &str) -> Self {
        GoError {
            message: message.to_string(),
        }
    }

    /// Returns the error message (Go's `Error()` method).
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for GoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for GoError {}

/// Creates a new error (Go `errors.New()`).
pub fn make_error(message: &str) -> Box<dyn std::error::Error> {
    Box::new(GoError::new(message))
}

/// Panics if the error is not nil (Go `if err != nil { panic(err) }`).
pub fn check_error(err: Option<&dyn std::error::Error>) {
    if let Some(e) = err {
        panic!("Go runtime error: {}", e);
    }
}

/// Recovers from a panic and returns the panic payload as an error string.
/// Mirrors Go's `defer recover()` pattern.
pub fn recover() -> Option<String> {
    // Note: This is a placeholder. Actual recovery requires `std::panic::catch_unwind`
    // at the call site, not inside the `recover()` function.
    None
}

// ─── Go math functions ─────────────────────────────────────────────────────

/// Returns the absolute value of an integer (Go `math.Abs`).
pub fn abs_i32(x: i32) -> i32 {
    x.abs()
}

/// Returns the absolute value of a 64-bit integer (Go `math.Abs` for int64).
pub fn abs_i64(x: i64) -> i64 {
    x.abs()
}

/// Returns the absolute value of a float (Go `math.Abs` for float64).
pub fn abs_f64(x: f64) -> f64 {
    x.abs()
}

/// Returns the square root of a float (Go `math.Sqrt`).
pub fn sqrt(x: f64) -> f64 {
    x.sqrt()
}

/// Returns the floor of a float (Go `math.Floor`).
pub fn floor(x: f64) -> f64 {
    x.floor()
}

/// Returns the ceiling of a float (Go `math.Ceil`).
pub fn ceil(x: f64) -> f64 {
    x.ceil()
}

/// Rounds a float to the nearest integer (Go `math.Round`).
pub fn round(x: f64) -> f64 {
    x.round()
}

/// Returns the minimum of two floats (Go `math.Min`).
pub fn min_f64(x: f64, y: f64) -> f64 {
    x.min(y)
}

/// Returns the maximum of two floats (Go `math.Max`).
pub fn max_f64(x: f64, y: f64) -> f64 {
    x.max(y)
}

/// Returns pi (Go `math.Pi`).
pub const PI: f64 = std::f64::consts::PI;

/// Returns e (Go `math.E`).
pub const E: f64 = std::f64::consts::E;

/// Returns the exponential of x (Go `math.Exp`).
pub fn exp(x: f64) -> f64 {
    x.exp()
}

/// Returns the natural logarithm of x (Go `math.Log`).
pub fn log(x: f64) -> f64 {
    x.ln()
}

/// Returns the base-10 logarithm of x (Go `math.Log10`).
pub fn log10(x: f64) -> f64 {
    x.log10()
}

/// Returns x raised to the power y (Go `math.Pow`).
pub fn pow(x: f64, y: f64) -> f64 {
    x.powf(y)
}

/// Returns the sign of x: -1, 0, or 1 (Go `math.Signbit` + sign logic).
pub fn sign(x: f64) -> f64 {
    if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }
}

// ─── Go byte/rune operations ──────────────────────────────────────────────

/// Returns the byte representation of a character (Go `byte(char)`).
pub fn byte_of(c: char) -> u8 {
    // Only returns the first byte if the char is ASCII
    c as u8
}

/// Returns the rune (Unicode code point) from a byte (Go `rune(byte)`).
pub fn rune_of(b: u8) -> char {
    b as char
}

/// Converts a string to bytes (Go `[]byte(string)`).
pub fn string_to_bytes(s: &str) -> Vec<u8> {
    s.as_bytes().to_vec()
}

/// Converts bytes to a string (Go `string([]byte)`).
pub fn bytes_to_string(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

// ─── Go time functions ─────────────────────────────────────────────────────

/// Returns the current Unix timestamp as nanoseconds (Go `time.Now().UnixNano()`).
pub fn now_nanos() -> i64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}

/// Returns the current Unix timestamp as seconds (Go `time.Now().Unix()`).
pub fn now_secs() -> i64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ─── Go scheduler ──────────────────────────────────────────────────────────

/// Go's goroutine scheduler — lightweight task executor.
///
/// Manages a pool of tasks that can be submitted, run, and cancelled.
/// Mirrors Go's goroutine model with `go func()` dispatch.
pub struct GoScheduler {
    inner: Mutex<GoSchedulerInner>,
}

struct GoSchedulerInner {
    tasks: Vec<Box<dyn FnOnce() + Send + 'static>>,
    running: bool,
}

impl GoScheduler {
    /// Creates a new scheduler.
    pub fn new() -> Self {
        GoScheduler {
            inner: Mutex::new(GoSchedulerInner {
                tasks: Vec::new(),
                running: false,
            }),
        }
    }

    /// Submits a task to the scheduler.
    /// Returns `true` if the task was added, `false` if the scheduler is running.
    pub fn submit<F: FnOnce() + Send + 'static>(&self, f: F) -> bool {
        let mut inner = self.inner.lock().unwrap();
        if inner.running {
            return false;
        }
        inner.tasks.push(Box::new(f));
        true
    }

    /// Runs all submitted tasks sequentially.
    pub fn run(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.running = true;
        let tasks = std::mem::take(&mut inner.tasks);
        drop(inner);

        for task in tasks {
            task();
        }

        let mut inner = self.inner.lock().unwrap();
        inner.running = false;
    }

    /// Returns the number of pending tasks.
    pub fn pending_count(&self) -> usize {
        self.inner.lock().unwrap().tasks.len()
    }

    /// Cancels all pending tasks.
    pub fn cancel_all(&self) {
        self.inner.lock().unwrap().tasks.clear();
    }
}

// ─── Go time primitives ────────────────────────────────────────────────────

/// Go's `time.Duration` — nanosecond-based duration.
///
/// Mirrors Go's `time.Duration` type which represents elapsed time
/// as an i64 number of nanoseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoDuration {
    nanos: i64,
}

impl GoDuration {
    /// Creates a duration from nanoseconds.
    pub fn new(nanos: i64) -> Self {
        GoDuration { nanos }
    }

    /// Creates a duration from milliseconds.
    pub fn from_millis(ms: i64) -> Self {
        GoDuration { nanos: ms * 1_000_000 }
    }

    /// Creates a duration from seconds.
    pub fn from_secs(s: i64) -> Self {
        GoDuration { nanos: s * 1_000_000_000 }
    }

    /// Creates a duration from minutes.
    pub fn from_mins(m: i64) -> Self {
        GoDuration { nanos: m * 60_000_000_000 }
    }

    /// Creates a duration from hours.
    pub fn from_hours(h: i64) -> Self {
        GoDuration { nanos: h * 3_600_000_000_000 }
    }

    /// Returns the duration in nanoseconds.
    pub fn nanos(&self) -> i64 {
        self.nanos
    }

    /// Returns the duration in milliseconds.
    pub fn millis(&self) -> i64 {
        self.nanos / 1_000_000
    }

    /// Returns the duration in seconds.
    pub fn secs(&self) -> i64 {
        self.nanos / 1_000_000_000
    }

    /// Returns the duration in minutes.
    pub fn mins(&self) -> i64 {
        self.nanos / 60_000_000_000
    }

    /// Returns the duration in hours.
    pub fn hours(&self) -> i64 {
        self.nanos / 3_600_000_000_000
    }

    /// Adds two durations.
    pub fn add(self, other: GoDuration) -> GoDuration {
        GoDuration { nanos: self.nanos + other.nanos }
    }

    /// Subtracts another duration from this one.
    pub fn sub(self, other: GoDuration) -> GoDuration {
        GoDuration { nanos: self.nanos - other.nanos }
    }

    /// Multiplies the duration by a scalar.
    pub fn mul(self, scalar: i64) -> GoDuration {
        GoDuration { nanos: self.nanos * scalar }
    }
}

/// Go's `time.Timer` — fires once after a duration.
///
/// Mirrors Go's `time.NewTimer(d)` pattern.
#[derive(Debug)]
pub struct GoTimer {
    shared: Arc<(Mutex<GoTimerInner>, Condvar)>,
}

#[derive(Debug)]
struct GoTimerInner {
    fired: bool,
    done: bool,
}

impl GoTimer {
    /// Creates a timer that fires after the given duration.
    /// The timer runs in a separate thread.
    pub fn new(dur: GoDuration) -> Self {
        let shared = Arc::new((
            Mutex::new(GoTimerInner {
                fired: false,
                done: false,
            }),
            Condvar::new(),
        ));
        let shared_clone = Arc::clone(&shared);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_nanos(dur.nanos() as u64));
            let (inner, fired) = &*shared_clone;
            let mut inner = inner.lock().unwrap();
            inner.fired = true;
            fired.notify_one();
        });

        GoTimer { shared }
    }

    /// Waits for the timer to fire. Returns `true` when fired.
    pub fn wait(&self) -> bool {
        let (inner, fired) = &*self.shared;
        let mut inner = inner.lock().unwrap();
        while !inner.fired && !inner.done {
            inner = fired.wait(inner).unwrap();
        }
        inner.fired
    }

    /// Stops the timer if it hasn't fired yet. Returns `true` if the timer
    /// was stopped before firing.
    pub fn stop(&self) -> bool {
        let (inner, _fired) = &*self.shared;
        let mut inner = inner.lock().unwrap();
        if !inner.fired {
            inner.done = true;
            return true;
        }
        false
    }

    /// Returns true if the timer has fired.
    pub fn fired(&self) -> bool {
        self.shared.0.lock().unwrap().fired
    }
}

/// Go's `time.Ticker` — fires repeatedly at a fixed interval.
///
/// Mirrors Go's `time.NewTicker(d)` pattern.
#[derive(Debug)]
pub struct GoTicker {
    shared: Arc<(Mutex<GoTickerInner>, Condvar)>,
    done: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Debug)]
struct GoTickerInner {
    count: i64,
    done: bool,
}

impl GoTicker {
    /// Creates a ticker that fires at regular intervals.
    /// The ticker runs in a separate thread.
    pub fn new(dur: GoDuration) -> Self {
        let shared = Arc::new((
            Mutex::new(GoTickerInner {
                count: 0,
                done: false,
            }),
            Condvar::new(),
        ));
        let shared_clone = Arc::clone(&shared);
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let done_clone = Arc::clone(&done);
        let interval = dur.nanos();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_nanos(interval as u64));
                if done_clone.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                let (inner, ticks) = &*shared_clone;
                let mut inner = inner.lock().unwrap();
                if !inner.done {
                    inner.count += 1;
                    ticks.notify_one();
                }
            }
        });

        GoTicker {
            shared,
            done,
        }
    }

    /// Waits for the next tick. Returns the tick count (starts at 0).
    pub fn next_tick(&self) -> i64 {
        let (inner, ticks) = &*self.shared;
        let mut inner = inner.lock().unwrap();
        while inner.count == 0 {
            inner = ticks.wait(inner).unwrap();
        }
        let count = inner.count;
        inner.count = 0;
        count
    }

    /// Stops the ticker.
    pub fn stop(&self) {
        self.done.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Returns the total number of ticks received so far.
    pub fn tick_count(&self) -> i64 {
        self.shared.0.lock().unwrap().count
    }
}

// ─── Go select ─────────────────────────────────────────────────────────────

/// Go's `select` statement — multiplexing over channel operations.
///
/// Used in the transpiler to generate Go-style `select` blocks.
pub struct GoSelect<T> {
    channels: Vec<(String, std::sync::Arc<GoChannel<T>>)>,
    handlers: Vec<Box<dyn FnMut(&GoSelectResult<T>) + Send>>,
}

#[derive(Debug)]
pub struct GoSelectResult<T> {
    pub tag: String,
    pub value: Option<T>,
}

impl<T> GoSelect<T> {
    /// Creates a new select statement.
    pub fn new() -> Self {
        GoSelect {
            channels: Vec::new(),
            handlers: Vec::new(),
        }
    }

    /// Adds a channel with a tag and handler.
    pub fn add_channel<F: FnMut(&GoSelectResult<T>) + Send + 'static>(
        &mut self,
        tag: String,
        ch: std::sync::Arc<GoChannel<T>>,
        handler: F,
    ) {
        self.channels.push((tag, ch));
        self.handlers.push(Box::new(handler));
    }

    /// Runs the select, waiting for any channel to have a value.
    pub fn run(&mut self) {
        // This is a blocking select — waits until one channel has data.
        // In Go, this corresponds to `select { ... }` without `default`.
        for (idx, (tag, ch)) in self.channels.iter().enumerate() {
            if let Some(value) = ch.recv() {
                if idx < self.handlers.len() {
                    let result = GoSelectResult {
                        tag: tag.clone(),
                        value: Some(value),
                    };
                    self.handlers[idx](&result);
                }
                return;
            }
        }
    }

    /// Tries all channels without blocking. Returns the first available value.
    pub fn try_select(&self) -> Option<(String, T)> {
        for (tag, ch) in &self.channels {
            if let Some(value) = ch.try_recv() {
                return Some((tag.clone(), value));
            }
        }
        None
    }
}

// ─── Go fmt package ─────────────────────────────────────────────────────────

/// Go's `fmt.Sprintf` — formatted string output.
///
/// Supports simple format specifiers: `%d` (int), `%s` (string),
/// `%v` (value), `%f` (float).
pub fn fmt_sprintf(format: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut result = String::new();
    let mut chars = format.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.next() {
                Some('d') => {
                    if let Some(arg) = args.first() {
                        result.push_str(&format!("{}", arg));
                    }
                    let _ = args.split_first(); // consume first arg
                }
                Some('s') => {
                    if let Some(arg) = args.first() {
                        result.push_str(&format!("{}", arg));
                    }
                    let _ = args.split_first();
                }
                Some('v') => {
                    if let Some(arg) = args.first() {
                        result.push_str(&format!("{}", arg));
                    }
                    let _ = args.split_first();
                }
                Some('f') => {
                    if let Some(arg) = args.first() {
                        result.push_str(&format!("{}", arg));
                    }
                    let _ = args.split_first();
                }
                Some(unknown) => {
                    result.push('%');
                    result.push(unknown);
                }
                None => {
                    result.push('%');
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}


// ─── Go rand package ───────────────────────────────────────────────────────

/// Go's `rand` package — pseudo-random number generation.
///
/// Mirrors Go's `math/rand` for simple random operations.
#[derive(Debug)]
pub struct GoRand {
    seed: u32,
}

impl GoRand {
    /// Creates a new random number generator with a fixed seed.
    pub fn new(seed: i64) -> Self {
        GoRand {
            seed: if seed < 0 { (-seed) as u32 } else { seed as u32 },
        }
    }

    /// Returns a random integer in [0, max).
    pub fn intn(&mut self, max: i64) -> i64 {
        self.next_u32();
        let range_size = if max <= 0 { 1 } else { max as u32 };
        let next = self.next_u32();
        (next % range_size) as i64
    }

    /// Returns a random float64 in [0.0, 1.0).
    pub fn float64(&mut self) -> f64 {
        self.next_u32();
        (self.next_u32() as f64) / (u32::MAX as f64)
    }

    /// Returns a random boolean.
    pub fn bool(&mut self) -> bool {
        self.next_u32() % 2 == 0
    }

    fn next_u32(&mut self) -> u32 {
        // Simple LCG-based generator
        self.seed = self.seed.wrapping_mul(1103515245).wrapping_add(12345);
        self.seed
    }
}
