// This is free and unencumbered software released into the public domain.

use crate::{Job, Spawner};
use alloc::{collections::VecDeque, format, sync::Arc};
use core::time::Duration;
use std::sync::{Condvar, Mutex, MutexGuard};

/// The built-in [`Spawner`]: a small, self-managing pool of threads.
///
/// Threads are started on demand, up to a maximum, and named `filer-N`; a
/// thread idle for longer than the idle timeout exits, so an idle pool
/// holds no threads. Jobs beyond the maximum wait in a queue, in order. A
/// job that panics ends only itself: its awaiting future sees that it never
/// completed (see [`run_on`](crate::run_on)), and the thread carries on.
///
/// Clones share one pool. Dropping the last clone leaves running threads to
/// finish their queued jobs and then exit after the idle timeout.
///
/// # Examples
///
/// ```
/// use core::time::Duration;
/// use filer_api::DefaultPool;
///
/// let pool = DefaultPool::new()
///     .with_max_threads(4)
///     .with_idle_timeout(Duration::from_secs(1));
/// assert_eq!(pool.threads(), 0);
/// ```
#[derive(Clone, Debug)]
pub struct DefaultPool {
    shared: Arc<Shared>,
}

struct Shared {
    state: Mutex<State>,
    /// Signaled when a job is queued for an idle thread.
    work: Condvar,
    max_threads: usize,
    idle_timeout: Duration,
}

#[derive(Default)]
struct State {
    jobs: VecDeque<Job>,
    threads: usize,
    idle: usize,
    next_name: usize,
}

impl core::fmt::Debug for Shared {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let state = self.lock();
        f.debug_struct("DefaultPool")
            .field("threads", &state.threads)
            .field("idle", &state.idle)
            .field("queued", &state.jobs.len())
            .field("max_threads", &self.max_threads)
            .field("idle_timeout", &self.idle_timeout)
            .finish()
    }
}

impl DefaultPool {
    /// The default maximum number of threads.
    pub const MAX_THREADS: usize = 64;

    /// The default idle timeout.
    pub const IDLE_TIMEOUT: Duration = Duration::from_secs(10);

    /// Returns a pool with the default limits, holding no threads yet.
    #[must_use]
    pub fn new() -> Self {
        Self::with(Self::MAX_THREADS, Self::IDLE_TIMEOUT)
    }

    fn with(max_threads: usize, idle_timeout: Duration) -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State::default()),
                work: Condvar::new(),
                max_threads: max_threads.max(1),
                idle_timeout,
            }),
        }
    }

    /// Returns a new pool like this one, with at most `max_threads`
    /// threads (at least one).
    #[must_use]
    pub fn with_max_threads(self, max_threads: usize) -> Self {
        Self::with(max_threads, self.shared.idle_timeout)
    }

    /// Returns a new pool like this one, whose threads exit after
    /// `idle_timeout` without work.
    #[must_use]
    pub fn with_idle_timeout(self, idle_timeout: Duration) -> Self {
        Self::with(self.shared.max_threads, idle_timeout)
    }

    /// Returns how many threads the pool holds now.
    #[must_use]
    pub fn threads(&self) -> usize {
        self.shared.lock().threads
    }
}

impl Default for DefaultPool {
    fn default() -> Self {
        Self::new()
    }
}

impl Spawner for DefaultPool {
    fn spawn_blocking(&self, job: Job) {
        let mut state = self.shared.lock();
        state.jobs.push_back(job);
        if state.idle > 0 {
            self.shared.work.notify_one();
            return;
        }
        if state.threads >= self.shared.max_threads {
            return; // A busy thread will take it.
        }
        state.threads += 1;
        state.next_name += 1;
        let name = format!("filer-{}", state.next_name);
        drop(state);
        let shared = self.shared.clone();
        let spawned = std::thread::Builder::new()
            .name(name)
            .spawn(move || shared.work());
        if spawned.is_err() {
            let mut state = self.shared.lock();
            state.threads -= 1;
            // With no thread to run them, run the queue here rather than
            // strand the jobs.
            if state.threads == 0 {
                let jobs = core::mem::take(&mut state.jobs);
                drop(state);
                jobs.into_iter().for_each(run);
            }
        }
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        // Jobs run unlocked, so a poisoned lock means a bug in this file;
        // the state is still consistent between statements.
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// A worker thread's loop: run jobs until idle for too long.
    fn work(&self) {
        let mut state = self.lock();
        loop {
            if let Some(job) = state.jobs.pop_front() {
                drop(state);
                run(job);
                state = self.lock();
                continue;
            }
            state.idle += 1;
            let (guard, timeout) = self
                .work
                .wait_timeout(state, self.idle_timeout)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state = guard;
            state.idle -= 1;
            if timeout.timed_out() && state.jobs.is_empty() {
                state.threads -= 1;
                return;
            }
        }
    }
}

/// Runs a job, containing a panic so that the thread survives it.
fn run(job: Job) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{driver::tests::block_on, run_on};
    use alloc::vec::Vec;
    use std::sync::{
        Barrier,
        atomic::{AtomicUsize, Ordering},
    };

    #[test]
    fn runs_jobs_and_returns_results() {
        let pool: Arc<dyn Spawner> = Arc::new(DefaultPool::new());
        assert_eq!(block_on(run_on(pool, || 6 * 7)), Some(42));
    }

    #[test]
    fn names_its_threads() {
        let pool: Arc<dyn Spawner> = Arc::new(DefaultPool::new());
        let name = block_on(run_on(pool, || {
            std::thread::current()
                .name()
                .map(alloc::string::String::from)
        }));
        assert!(name.flatten().unwrap().starts_with("filer-"));
    }

    #[test]
    fn queues_beyond_its_maximum() {
        let pool = DefaultPool::new().with_max_threads(2);
        let spawner: Arc<dyn Spawner> = Arc::new(pool.clone());
        let done = Arc::new(AtomicUsize::new(0));
        let futures: Vec<_> = (0..20)
            .map(|_| {
                let done = done.clone();
                run_on(spawner.clone(), move || {
                    std::thread::sleep(Duration::from_millis(1));
                    done.fetch_add(1, Ordering::SeqCst);
                })
            })
            .collect();
        for future in futures {
            assert_eq!(block_on(future), Some(()));
        }
        assert_eq!(done.load(Ordering::SeqCst), 20);
        assert!(pool.threads() <= 2);
    }

    #[test]
    fn runs_jobs_concurrently() {
        // Two jobs that wait for each other finish only if both run at
        // once. Each future is awaited on its own thread, since a job is
        // submitted only when its future is first polled.
        let spawner: Arc<dyn Spawner> = Arc::new(DefaultPool::new().with_max_threads(2));
        let barrier = Arc::new(Barrier::new(2));
        std::thread::scope(|scope| {
            let waiters: Vec<_> = (0..2)
                .map(|_| {
                    let barrier = barrier.clone();
                    let future = run_on(spawner.clone(), move || {
                        barrier.wait();
                    });
                    scope.spawn(move || block_on(future))
                })
                .collect();
            for waiter in waiters {
                assert_eq!(waiter.join().unwrap(), Some(()));
            }
        });
    }

    #[test]
    fn survives_panicking_jobs() {
        let pool = DefaultPool::new().with_max_threads(1);
        let spawner: Arc<dyn Spawner> = Arc::new(pool.clone());
        let panicked = run_on(spawner.clone(), || -> u8 { panic!("job panicked") });
        assert_eq!(block_on(panicked), None);
        assert_eq!(block_on(run_on(spawner, || 1)), Some(1));
        assert_eq!(pool.threads(), 1);
    }

    #[test]
    fn retires_idle_threads() {
        let pool = DefaultPool::new().with_idle_timeout(Duration::from_millis(20));
        let spawner: Arc<dyn Spawner> = Arc::new(pool.clone());
        block_on(run_on(spawner, || ()));
        assert_eq!(pool.threads(), 1);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while pool.threads() > 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(pool.threads(), 0);
    }
}
