// This is free and unencumbered software released into the public domain.

use crate::Spawner;
use alloc::{boxed::Box, sync::Arc};
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use std::sync::Mutex;

/// Returns a future that runs `job` on `spawner` and resolves to its
/// result, or to `None` if the job was dropped without completing (it
/// panicked, or the spawner could not run it).
///
/// Like every Filer operation, the job is submitted when the future is
/// first polled, not when it is created. Once submitted, it runs to
/// completion even if the future is dropped; dropping only discards the
/// result.
///
/// # Examples
///
/// ```
/// use filer_api::{DefaultPool, run_on};
/// use std::sync::Arc;
///
/// let pool = Arc::new(DefaultPool::new());
/// # filer_api_block_on(async {
/// let answer = run_on(pool, || 6 * 7).await;
/// assert_eq!(answer, Some(42));
/// # });
/// # fn filer_api_block_on<F: core::future::Future>(f: F) -> F::Output {
/// #     let mut f = core::pin::pin!(f);
/// #     let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
/// #     loop {
/// #         if let core::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
/// #             return v;
/// #         }
/// #     }
/// # }
/// ```
pub fn run_on<T, F>(spawner: Arc<dyn Spawner>, job: F) -> RunOn<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    RunOn(State::Unstarted {
        spawner,
        job: Box::new(job),
    })
}

/// The future returned by [`run_on`].
#[must_use = "futures do nothing unless polled"]
pub struct RunOn<T>(State<T>);

enum State<T> {
    Unstarted {
        spawner: Arc<dyn Spawner>,
        job: Box<dyn FnOnce() -> T + Send>,
    },
    Started(Arc<Mutex<Slot<T>>>),
    Done,
}

/// Where a job's result meets the future awaiting it.
struct Slot<T> {
    value: Option<T>,
    /// Set when the job finished or was dropped without a value.
    closed: bool,
    waker: Option<Waker>,
}

/// The job's end of a [`Slot`]: delivers the value, or, if dropped first
/// (the job panicked or never ran), closes the slot empty.
struct Sender<T>(Arc<Mutex<Slot<T>>>);

impl<T> Sender<T> {
    fn send(self, value: T) {
        lock(&self.0).value = Some(value);
        // Dropping `self` closes the slot and wakes the receiver.
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let waker = {
            let mut slot = lock(&self.0);
            slot.closed = true;
            slot.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

/// Locks a slot, ignoring poisoning: a slot's state stays consistent even
/// if a holder panicked, since every update is a single assignment.
fn lock<T>(slot: &Mutex<Slot<T>>) -> std::sync::MutexGuard<'_, Slot<T>> {
    slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl<T: Send + 'static> Future for RunOn<T> {
    type Output = Option<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        if let State::Unstarted { .. } = self.0 {
            let State::Unstarted { spawner, job } = core::mem::replace(&mut self.0, State::Done)
            else {
                unreachable!("checked above");
            };
            let slot = Arc::new(Mutex::new(Slot {
                value: None,
                closed: false,
                waker: Some(cx.waker().clone()),
            }));
            let sender = Sender(slot.clone());
            spawner.spawn_blocking(Box::new(move || sender.send(job())));
            self.0 = State::Started(slot);
        }
        let State::Started(slot) = &self.0 else {
            panic!("`RunOn` polled after completion");
        };
        let mut slot = lock(slot);
        if !slot.closed {
            slot.waker = Some(cx.waker().clone());
            return Poll::Pending;
        }
        let value = slot.value.take();
        drop(slot);
        self.0 = State::Done;
        Poll::Ready(value)
    }
}

impl<T> core::fmt::Debug for RunOn<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let state = match self.0 {
            State::Unstarted { .. } => "unstarted",
            State::Started(_) => "started",
            State::Done => "done",
        };
        f.debug_tuple("RunOn").field(&state).finish()
    }
}
