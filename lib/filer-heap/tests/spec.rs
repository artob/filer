// This is free and unencumbered software released into the public domain.

//! The Filer specification, run against a heap.

use core::future::Future;
use filer_api::{Completion, Dir, Driver, DriverId, Op, Resolve};
use filer_heap::Heap;
use filer_spec::{Profile, Tree, TreeEntry};

/// The builder cannot create FIFOs or non-UTF-8 names yet.
fn profile() -> Profile {
    Profile::default()
        .with_fifos(false)
        .with_foreign_names(false)
}

fn build(tree: &Tree) -> Heap {
    let mut builder = Heap::builder();
    for entry in tree.entries() {
        builder = match entry {
            TreeEntry::Dir(path) => builder.dir(path.clone()),
            TreeEntry::File(path, contents) => builder.file(path.clone(), contents.clone()),
            TreeEntry::Symlink(path, target) => builder.symlink(path.clone(), target.as_str()),
            entry => panic!("not in this profile: {entry:?}"),
        };
    }
    builder.build()
}

#[test]
fn passes_the_specification() {
    filer_spec::run_blocking(profile(), |tree| build(tree).root_dir()).assert_passed();
}

/// A driver that silently loosens every `Open` policy to the default, which
/// the specification must catch.
#[derive(Clone)]
struct Loosening(Heap);

impl Driver for Loosening {
    fn id(&self) -> DriverId {
        self.0.id()
    }

    fn submit(&self, mut op: Op) -> impl Future<Output = Completion> + Send + use<> {
        if let Op::Open { how, .. } = &mut op {
            how.resolve = Resolve::default();
        }
        self.0.submit(op)
    }
}

#[test]
fn catches_a_driver_that_loosens_policies() {
    let report = filer_spec::run_blocking(profile(), |tree| {
        let heap = build(tree);
        let root = heap.root();
        Dir::new(Loosening(heap), root).unwrap()
    });
    let failed: Vec<&str> = report.failures().map(|(name, _)| name).collect();
    assert!(failed.contains(&"denies_symlinks"), "{report}");
    assert!(failed.contains(&"treats_the_handle_as_root"), "{report}");
}
