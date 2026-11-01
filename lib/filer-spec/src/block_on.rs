// This is free and unencumbered software released into the public domain.

use core::{
    future::Future,
    task::{Context, Poll},
};

/// Runs `future` to completion on the current thread.
///
/// Under `std`, the thread sleeps until the future's waker is called, so
/// drivers that complete on other threads work without spinning. Without
/// `std`, it polls in a loop.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = core::pin::pin!(future);
    let waker = waker();
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
            return output;
        }
        park();
    }
}

#[cfg(feature = "std")]
fn waker() -> core::task::Waker {
    /// Wakes a parked thread.
    struct Unpark(std::thread::Thread);

    impl std::task::Wake for Unpark {
        fn wake(self: alloc::sync::Arc<Self>) {
            self.0.unpark();
        }
    }

    core::task::Waker::from(alloc::sync::Arc::new(Unpark(std::thread::current())))
}

#[cfg(feature = "std")]
fn park() {
    std::thread::park();
}

#[cfg(not(feature = "std"))]
fn waker() -> core::task::Waker {
    core::task::Waker::noop().clone()
}

#[cfg(not(feature = "std"))]
fn park() {
    core::hint::spin_loop();
}
