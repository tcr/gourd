//! Go's synchronization primitives.
//!
//! Provides mutex, reference-counted pointers with interior mutability,
//! wait groups, read-write locks, and one-shot execution.

use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

// ─── GoMutex<T> ────────────────────────────────────────────────────────────

/// Go's `sync.Mutex` — guards a value behind exclusive access.
///
/// Used to share mutable state across concurrent goroutines.
///
/// ```
/// use gourd::prelude::GoMutex;
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
/// use gourd::prelude::GoRc;
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
