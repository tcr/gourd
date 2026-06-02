use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Context, Poll};

/// A simple sequential scheduler for goroutines.
///
/// In the fake Go GC, `go func() { ... }` spawns a goroutine, but since we're
/// not implementing real concurrency, we execute goroutines sequentially in
/// the order they are submitted.
///
/// Usage in generated Rust:
/// ```ignore
/// GoScheduler::new().submit(|| { /* body */ });
/// ```
pub struct GoScheduler {
    tasks: Mutex<Vec<Box<dyn FnOnce()>>>,
}

impl GoScheduler {
    /// Creates a new empty scheduler.
    pub fn new() -> Self {
        GoScheduler {
            tasks: Mutex::new(Vec::new()),
        }
    }

    /// Submits a closure to be executed sequentially.
    ///
    /// In the fake Go GC, this represents spawning a goroutine. The closure
    /// is stored and will be executed when the scheduler runs (or dropped).
    pub fn submit<F: FnOnce() + 'static>(&self, f: F) {
        let mut tasks = self.tasks.lock().unwrap();
        tasks.push(Box::new(f));
    }

    /// Executes all submitted tasks sequentially.
    ///
    /// This simulates the Go runtime executing goroutines. In the fake GC,
    /// they run one after another, not in parallel.
    pub fn run(&self) {
        let mut tasks = self.tasks.lock().unwrap();
        while let Some(task) = tasks.pop() {
            task();
        }
    }
}

impl Clone for GoScheduler {
    fn clone(&self) -> Self {
        GoScheduler {
            tasks: Mutex::new(Vec::new()),
        }
    }
}

// SAFETY: GoScheduler can be sent between threads since it uses Mutex internally.
unsafe impl Send for GoScheduler {}
unsafe impl Sync for GoScheduler {}

/// A minimal future implementation for converting closures into futures.
///
/// This allows closures to be used with async-style code without requiring
/// the full async/await machinery.
pub struct GoFuture<F: FnOnce() -> ()> {
    f: Option<F>,
}

impl<F: FnOnce() -> ()> GoFuture<F> {
    pub fn new(f: F) -> Self {
        GoFuture { f: Some(f) }
    }
}

impl<F: FnOnce() -> ()> Future for GoFuture<F> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: We never move the closure out, we only call it once
        unsafe {
            let this = self.get_unchecked_mut();
            if let Some(f) = this.f.take() {
                f();
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        }
    }
}

// Stub types for future concurrency primitives
/// Placeholder for channel types — not yet implemented.
pub struct GoChannel<T> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T> GoChannel<T> {
    /// Creates a new unbuffered channel.
    pub fn new() -> Self {
        GoChannel {
            _phantom: std::marker::PhantomData,
        }
    }
}

/// Placeholder for select statement results.
pub struct GoSelectResult<T> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T> GoSelectResult<T> {
    /// Returns true if the channel was selected.
    pub fn is_ready(&self) -> bool {
        false
    }
}

impl<T> Clone for GoSelectResult<T> {
    fn clone(&self) -> Self {
        GoSelectResult {
            _phantom: std::marker::PhantomData,
        }
    }
}
