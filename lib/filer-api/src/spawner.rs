// This is free and unencumbered software released into the public domain.

use alloc::boxed::Box;

/// A job for a [`Spawner`]: blocking work, such as a system call.
pub type Job = Box<dyn FnOnce() + Send + 'static>;

/// Runs blocking jobs off the async executor, on threads that may block.
///
/// Drivers that make blocking system calls take a spawner, so that a slow
/// call (a cold disk, a hung network mount) occupies a worker thread rather
/// than an executor thread. The trait names no async runtime: the built-in
/// `DefaultPool` (under `std`) implements it, and so can a runtime's own
/// blocking pool, such as Tokio's `spawn_blocking`. Under `std`, `run_on`
/// turns a job into a future of its result.
///
/// # Examples
///
/// ```
/// use filer_api::{Job, Spawner};
///
/// /// Runs every job on a new thread.
/// struct ThreadPerJob;
///
/// impl Spawner for ThreadPerJob {
///     fn spawn_blocking(&self, job: Job) {
///         std::thread::spawn(job);
///     }
/// }
/// ```
pub trait Spawner: Send + Sync + 'static {
    /// Runs `job` soon, on a thread where blocking is fine.
    ///
    /// A spawner may drop a job without running it only if it cannot run
    /// it at all (for example, while shutting down); whoever awaits the
    /// job's result then sees that it never ran.
    fn spawn_blocking(&self, job: Job);
}
