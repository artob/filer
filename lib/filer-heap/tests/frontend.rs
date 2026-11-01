// This is free and unencumbered software released into the public domain.

//! The `filer-api` frontend over a heap.

use core::{
    future::Future,
    task::{Context, Poll, Waker},
};
use filer_api::{ErrorKind, Rights};
use filer_heap::Heap;

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = core::pin::pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
            return output;
        }
    }
}

fn heap() -> Heap {
    Heap::builder()
        .file("config/app.toml", "debug = true")
        .symlink("config/current", "app.toml")
        .symlink("config/root", "/config/app.toml")
        .dir("empty")
        .build()
}

#[test]
fn inherits_rights_and_labels() {
    let root = heap().root_dir().with_label("<heap>");
    let blind = root.restrict(Rights::all() - Rights::READ);
    let config = block_on(blind.open_dir("config")).unwrap();
    assert_eq!(config.rights(), blind.rights());
    let error = block_on(config.open_file("app.toml")).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::RightsDenied);
    assert_eq!(error.label(), Some("<heap>"));
    let error = block_on(root.open_file("config/nope")).unwrap_err();
    assert_eq!(
        error.to_string(),
        "open \"config/nope\" in <heap>: not found"
    );
}

#[test]
fn erases_the_driver_type() {
    let root = heap().root_dir().into_dyn();
    let file = block_on(root.open_file("config/app.toml")).unwrap();
    assert_eq!(block_on(file.read_to_end()).unwrap(), b"debug = true");
}
