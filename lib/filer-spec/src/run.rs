// This is free and unencumbered software released into the public domain.

use crate::{
    Profile, Report, Tree,
    cases::{self, Ctx},
};
use filer_api::{Dir, Driver};

/// Runs the specification against a driver, returning each case's verdict.
///
/// `setup` creates the given [`Tree`] in a fresh driver instance and
/// returns its root directory, with all rights. It is called twice, so
/// that cases can check that two instances refuse each other's handles and
/// names; each call must use a new driver instance.
///
/// Cases only read, so they share the trees. Call
/// [`Report::assert_passed`] on the result to fail a test with every
/// failure listed, or print the report to see each case.
///
/// Under `std`, [`run_blocking`](crate::run_blocking) drives this to
/// completion on the current thread.
pub async fn run<D: Driver>(profile: Profile, mut setup: impl FnMut(&Tree) -> Dir<D>) -> Report {
    let tree = Tree::standard(&profile);
    let ctx = Ctx {
        root: setup(&tree),
        other: setup(&tree),
        profile,
    };
    let mut report = Report::default();
    macro_rules! case {
        ($name:ident) => {
            report
                .cases
                .push((stringify!($name), cases::$name(&ctx).await));
        };
    }
    case!(reads_files);
    case!(opens_relative_to_directories);
    case!(reports_open_errors);
    case!(follows_symlinks_beneath);
    case!(treats_the_handle_as_root);
    case!(denies_symlinks);
    case!(stops_loops_and_dangling_links);
    case!(reads_metadata);
    case!(reads_links);
    case!(lists_directories);
    case!(lists_large_directories);
    case!(enforces_rights);
    case!(rejects_other_drivers_handles);
    case!(refuses_fifos);
    case!(lists_and_stats_foreign_names);
    report
}

/// Runs the specification to completion on the current thread; see
/// [`run`].
pub fn run_blocking<D: Driver>(profile: Profile, setup: impl FnMut(&Tree) -> Dir<D>) -> Report {
    crate::block_on(run(profile, setup))
}
