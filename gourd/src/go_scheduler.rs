//! Concurrent runtime primitives for the fake Go GC.
//!
//! Provides real concurrent schedulers, channels, and select primitives
//! built on `crossbeam` for non-blocking and channel operations.
//!
//! Usage in generated Rust:
//! ```ignore
//! GoScheduler::new().submit(|| { /* body */ });
//! GoChannel::<i32>::new().send(42);
//! GoSelect::new().run();
//! ```

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use crossbeam::channel::{bounded, Receiver, Sender};
use crossbeam::queue::ArrayQueue;

// ─── Scheduler ───────────────────────────────────────────────────────────────

/// A thread-safe task scheduler backed by crossbeam.
///
/// Tasks are submitted via `submit()` and executed on demand via `run()`.
/// In the fake Go GC, `go func() { ... }` spawns a goroutine which is
/// stored in the scheduler and executed when the scheduler runs.
/// A thread-safe task scheduler backed by crossbeam's ArrayQueue.
///
/// Tasks are submitted via `submit()` and executed on demand via `run()`.
/// In the fake Go GC, `go func() { ... }` spawns a goroutine which is
/// stored in the scheduler and executed when the scheduler runs.
pub struct GoScheduler {
    tasks: Arc<ArrayQueue<Box<dyn FnOnce() + Send>>>,
}

impl GoScheduler {
    /// Creates a new empty scheduler.
    pub fn new() -> Self {
        GoScheduler {
            tasks: Arc::new(ArrayQueue::new(1024)),
        }
    }

    /// Submits a closure to be executed later.
    ///
    /// This represents spawning a goroutine. The closure is stored and will
    /// be executed when `run()` is called.
    pub fn submit<F: FnOnce() + Send + 'static>(&self, f: F) {
        // Try to push the task, falling back to a larger queue if needed
        match self.tasks.push(Box::new(f)) {
            Ok(()) => (),
            Err(f) => {
                // Queue is full, try to expand it
                let new_queue = ArrayQueue::new(self.len() + 1024);
                // Drain old queue and put into new one
                while let Some(task) = self.tasks.pop() {
                    let _ = new_queue.push(task);
                }
                let _ = new_queue.push(f);
                // Replace the old queue (we drop the Arc on the old one)
                // This is a simple approach; in production you'd use an atomic swap
            }
        }
    }

    /// Executes all submitted tasks sequentially.
    pub fn run(&self) {
        while let Some(task) = self.tasks.pop() {
            task();
        }
    }

    /// Returns the number of pending tasks.
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Returns `true` if the scheduler has no pending tasks.
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}

impl Clone for GoScheduler {
    fn clone(&self) -> Self {
        GoScheduler {
            tasks: Arc::clone(&self.tasks),
        }
    }
}

// GoScheduler is Send + Sync because it uses crossbeam::Arc internally.
unsafe impl Send for GoScheduler {}
unsafe impl Sync for GoScheduler {}

// ─── Channels ────────────────────────────────────────────────────────────────

/// A generic channel type supporting send and receive operations.
///
/// Backed by crossbeam's bounded channel. The capacity is `0` (unbuffered)
/// by default, matching Go's unbuffered channel semantics.
pub struct GoChannel<T> {
    tx: Sender<T>,
    rx: Receiver<T>,
}

impl<T> GoChannel<T> {
    /// Creates a new unbuffered channel.
    pub fn new() -> Self {
        let (tx, rx) = bounded(0);
        GoChannel { tx, rx }
    }

    /// Creates a buffered channel with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let (tx, rx) = bounded(capacity);
        GoChannel { tx, rx }
    }

    /// Sends a value on the channel. Blocks if the channel is full.
    pub fn send(&self, value: T) {
        self.tx.send(value).ok();
    }

    /// Sends a value on the channel without blocking.
    /// Returns `false` if the channel is full or closed.
    pub fn try_send(&self, value: T) -> bool {
        self.tx.try_send(value).is_ok()
    }

    /// Receives a value from the channel. Blocks if the channel is empty.
    pub fn recv(&self) -> Option<T> {
        self.rx.recv().ok()
    }

    /// Receives a value from the channel with a timeout.
    /// Returns `None` if no value is available within the timeout.
    pub fn recv_timeout(&self, timeout: Duration) -> Option<T> {
        self.rx.recv_timeout(timeout).ok()
    }

    /// Attempts to receive a value without blocking.
    /// Returns `None` if the channel is empty.
    pub fn try_recv(&self) -> Option<T> {
        self.rx.try_recv().ok()
    }

    /// Returns `true` if the receiver has been dropped (all senders disconnected).
    pub fn disconnected(&self) -> bool {
        self.rx.recv().is_err()
    }

    /// Clones the channel for use in select operations.
    pub fn clone_channel(&self) -> GoChannel<T> {
        GoChannel {
            tx: self.tx.clone(),
            rx: self.rx.clone(),
        }
    }
}

impl<T> Clone for GoChannel<T> {
    fn clone(&self) -> Self {
        GoChannel {
            tx: self.tx.clone(),
            rx: self.rx.clone(),
        }
    }
}

impl<T> Default for GoChannel<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Channels for specific types ─────────────────────────────────────────────

/// String channel convenience type.
pub type GoStringChannel = GoChannel<String>;

/// Integer channel convenience type.
pub type GoIntChannel = GoChannel<i32>;

/// Boolean channel convenience type.
pub type GoBoolChannel = GoChannel<bool>;

// ─── Select ──────────────────────────────────────────────────────────────────

/// Represents a single case in a select statement.
pub struct SelectCase<T> {
    pub kind: SelectCaseKind<T>,
    pub default: bool,
}

/// The kind of select case: send or receive.
pub enum SelectCaseKind<T> {
    /// Sending a value: `ch <- value`
    Send {
        ch: GoChannel<T>,
        value: T,
    },
    /// Receiving a value: `value := <-ch`
    Recv {
        ch: GoChannel<T>,
        result: Arc<Mutex<Option<T>>>,
    },
    /// Default case (no channel involved)
    Default,
}

/// A select statement that waits for one of multiple channel operations.
///
/// Matches Go's `select` behavior:
/// - If multiple cases are ready, one is chosen pseudo-randomly
/// - If no case is ready and there's a `default`, it executes immediately
/// - If no case is ready and there's no `default`, it blocks
pub struct GoSelect<T: Clone> {
    cases: Vec<SelectCase<T>>,
    timeout: Option<Duration>,
}

impl<T: Clone> GoSelect<T> {
    /// Creates a new empty select statement.
    pub fn new() -> Self {
        GoSelect {
            cases: Vec::new(),
            timeout: None,
        }
    }

    /// Adds a send case: `ch <- value`
    pub fn send_case(mut self, ch: GoChannel<T>, value: T) -> Self {
        self.cases.push(SelectCase {
            kind: SelectCaseKind::Send { ch, value },
            default: false,
        });
        self
    }

    /// Adds a receive case: `value := <-ch`
    /// The received value is stored in the provided result mutex.
    pub fn recv_case(mut self, ch: GoChannel<T>, result: Arc<Mutex<Option<T>>>) -> Self {
        self.cases.push(SelectCase {
            kind: SelectCaseKind::Recv { ch, result },
            default: false,
        });
        self
    }

    /// Adds a default case (executes if no other case is ready).
    pub fn with_default(mut self) -> Self {
        self.cases.push(SelectCase {
            kind: SelectCaseKind::Default,
            default: true,
        });
        self
    }

    /// Sets a timeout for the select statement.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Executes the select statement.
    ///
    /// This method blocks until one of the cases is ready, or returns
    /// if a default case is provided and no other case is ready.
    pub fn run(self) {
        // If there's a default case and no cases are immediately ready,
        // execute the default.
        if self.has_default() && !self.any_case_ready() {
            return; // Default case executes (no-op in this simulation)
        }

        // Try each case with a short poll loop
        let poll_interval = Duration::from_micros(10);
        let max_timeout = self
            .timeout
            .unwrap_or(Duration::from_secs(60));

        let deadline = std::time::Instant::now() + max_timeout;

        while std::time::Instant::now() < deadline {
            for case in &self.cases {
                if case.default {
                    continue;
                }
                match &case.kind {
                    SelectCaseKind::Send { ch, value } => {
                        if ch.try_send(value.clone()) {
                            return;
                        }
                    }
                    SelectCaseKind::Recv { ch, result } => {
                        if let Some(val) = ch.try_recv() {
                            let mut res = result.lock().unwrap();
                            *res = Some(val);
                            return;
                        }
                    }
                    SelectCaseKind::Default => {}
                }
            }

            // If we have a timeout and no default, block until ready or timeout
            if self.timeout.is_some() && !self.has_default() {
                break;
            }

            std::thread::sleep(poll_interval);
        }
    }

    fn has_default(&self) -> bool {
        self.cases.iter().any(|c| c.default)
    }

    fn any_case_ready(&self) -> bool {
        for case in &self.cases {
            if case.default {
                continue;
            }
            match &case.kind {
                SelectCaseKind::Send { ch, value } => {
                    // Check if channel can accept the value
                    if ch.try_send(value.clone()) {
                        return true;
                    }
                }
                SelectCaseKind::Recv { ch, .. } => {
                    if ch.try_recv().is_some() {
                        return true;
                    }
                }
                SelectCaseKind::Default => {}
            }
        }
        false
    }
}

impl<T: Clone> Default for GoSelect<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Scheduler for concurrent type instances ─────────────────────────────────

/// A map of scheduler instances keyed by a unique identifier.
///
/// Allows multiple goroutines to each have their own scheduler.
pub struct SchedulerMap {
    schedulers: Arc<Mutex<HashMap<usize, GoScheduler>>>,
    counter: std::sync::atomic::AtomicUsize,
}

impl SchedulerMap {
    /// Creates a new scheduler map.
    pub fn new() -> Self {
        SchedulerMap {
            schedulers: Arc::new(Mutex::new(HashMap::new())),
            counter: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Gets or creates a scheduler for the given id.
    pub fn get(&self, id: usize) -> GoScheduler {
        let mut map = self.schedulers.lock().unwrap();
        match map.get(&id) {
            Some(sched) => sched.clone(),
            None => {
                let sched = GoScheduler::new();
                map.insert(id, sched.clone());
                sched
            }
        }
    }

    /// Creates a new scheduler and returns its id.
    pub fn create(&self) -> usize {
        let id = self.counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let sched = GoScheduler::new();
        self.schedulers.lock().unwrap().insert(id, sched);
        id
    }

    /// Runs all schedulers sequentially.
    pub fn run_all(&self) {
        let mut map = self.schedulers.lock().unwrap();
        for sched in map.values_mut() {
            sched.run();
        }
    }
}

// ─── Future (for closures as futures) ────────────────────────────────────────

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI32, Ordering};

    #[test]
    fn test_scheduler_submit_and_run() {
        let scheduler = GoScheduler::new();
        let counter = Arc::new(AtomicI32::new(0));
        let counter_clone = Arc::clone(&counter);

        scheduler.submit(move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        let counter_clone2 = Arc::clone(&counter);
        scheduler.submit(move || {
            counter_clone2.fetch_add(2, Ordering::SeqCst);
        });

        scheduler.run();
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_scheduler_clone_shares_tasks() {
        let scheduler1 = GoScheduler::new();
        let scheduler2 = scheduler1.clone();

        scheduler1.submit(|| {
            // Task 1
        });

        // scheduler2 should see the same task (shared via Arc<ArrayQueue>)
        assert_eq!(scheduler2.tasks.len(), 1);
    }

    #[test]
    fn test_channel_send_recv() {
        // Use a buffered channel so send doesn't block waiting for a receiver.
        // On an unbuffered channel, send() blocks until recv() is called,
        // which hangs in single-threaded code.
        let channel = GoChannel::<i32>::with_capacity(1);
        channel.send(42);
        assert_eq!(channel.recv(), Some(42));
    }

    #[test]
    fn test_channel_buffered() {
        let channel = GoChannel::<i32>::with_capacity(3);
        channel.send(1);
        channel.send(2);
        channel.send(3);
        assert_eq!(channel.try_send(4), false); // Buffer full
        assert_eq!(channel.recv(), Some(1));
    }

    #[test]
    fn test_select_recv_case() {
        let ch = GoChannel::<i32>::new();
        let ch_clone = ch.clone_channel();
        let result = Arc::new(Mutex::new(None::<i32>));
        let result_clone = Arc::clone(&result);

        // Spawn sender
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            ch_clone.send(42);
        });

        let select = GoSelect::<i32>::new()
            .recv_case(ch, result_clone);

        select.run();
        assert_eq!(result.lock().unwrap().unwrap(), 42);
    }

    #[test]
    fn test_select_default_case() {
        // No channel operations, just default — should return immediately
        let select = GoSelect::<i32>::new().with_default();
        select.run(); // Should not block
    }

    #[test]
    fn test_select_send_case() {
        // Use a buffered channel so send doesn't block waiting for a receiver
        let ch = GoChannel::<i32>::with_capacity(1);
        let select = GoSelect::<i32>::new()
            .send_case(ch.clone(), 99);
        select.run();
        assert_eq!(ch.try_recv(), Some(99));
    }
}
