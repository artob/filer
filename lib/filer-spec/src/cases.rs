// This is free and unencumbered software released into the public domain.

//! The specification's cases, each checking one part of the driver
//! contract through the frontend.

use crate::{
    Profile, Verdict,
    tree::{FOREIGN_NAME, MANY},
};
use alloc::{format, string::String, string::ToString, vec::Vec};
use filer_api::{
    Completion, Dir, Driver, EntryName, Error, ErrorKind, FileKind, MetaMask, Op, Output, Resolve,
    Rights, Symlinks, Target,
};

/// Everything a case needs.
pub(crate) struct Ctx<D: Driver> {
    pub(crate) root: Dir<D>,
    /// The root of a second, independent instance of the same driver.
    pub(crate) other: Dir<D>,
    pub(crate) profile: Profile,
}

/// Fails the case unless `cond` holds.
macro_rules! check {
    ($cond:expr, $($message:tt)+) => {
        if !$cond {
            return Verdict::Fail(format!($($message)+));
        }
    };
}

/// Fails the case unless `left == right`.
macro_rules! check_eq {
    ($left:expr, $right:expr) => {
        // A `match` keeps temporaries alive, as in `assert_eq!`.
        match (&$left, &$right) {
            (left, right) => {
                if left != right {
                    return Verdict::Fail(format!(
                        "{} == {}: {left:?} != {right:?}",
                        stringify!($left),
                        stringify!($right),
                    ));
                }
            },
        }
    };
}

/// Returns the success value, or fails the case with the error.
macro_rules! ok {
    ($result:expr) => {
        match $result {
            Ok(value) => value,
            Err(error) => {
                return Verdict::Fail(format!(
                    "{}: unexpected error: {error}",
                    stringify!($result)
                ))
            },
        }
    };
}

/// Fails the case unless `result` is an error of `kind`; returns the error.
macro_rules! expect_error {
    ($result:expr, $kind:expr) => {
        match $result {
            Ok(_) => {
                return Verdict::Fail(format!(
                    "{}: succeeded, expected {:?}",
                    stringify!($result),
                    $kind
                ))
            },
            Err(error) if error.kind() == $kind => error,
            Err(error) => {
                return Verdict::Fail(format!(
                    "{}: expected {:?}, got {error}",
                    stringify!($result),
                    $kind
                ))
            },
        }
    };
}

/// Skips the case unless the profile allows symlinks.
macro_rules! need_symlinks {
    ($ctx:expr) => {
        if !$ctx.profile.symlinks {
            return Verdict::Skip("no symlinks in this profile");
        }
    };
}

/// Lists `dir` completely, sorted by name.
async fn list<D: Driver>(dir: &Dir<D>) -> Result<Vec<(String, Option<FileKind>)>, Error> {
    let mut entries = dir.entries()?;
    let mut names = Vec::new();
    while let Some(entry) = entries.next().await {
        let entry = entry?;
        names.push((entry.name.to_string(), entry.kind));
    }
    names.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(names)
}

pub(crate) async fn reads_files<D: Driver>(ctx: &Ctx<D>) -> Verdict {
    let file = ok!(ctx.root.open_file("a/b/c.txt").await);
    check_eq!(ok!(file.read_to_end().await), b"contents".to_vec());
    let filer_api::BufResult(n, buf) = file.read_at(Vec::with_capacity(2), 3).await;
    check_eq!(ok!(n), 2);
    check_eq!(buf, b"te".to_vec());
    let filer_api::BufResult(n, _) = file.read_at(Vec::with_capacity(4), 100).await;
    check_eq!(ok!(n), 0);
    let empty = ok!(ctx.root.open_file("a/z.txt").await);
    check_eq!(ok!(empty.read_to_end().await), Vec::<u8>::new());
    Verdict::Pass
}

pub(crate) async fn opens_relative_to_directories<D: Driver>(ctx: &Ctx<D>) -> Verdict {
    let a = ok!(ctx.root.open_dir("a").await);
    let file = ok!(a.open_file("b/c.txt").await);
    check_eq!(ok!(file.read_to_end().await), b"contents".to_vec());
    let again = ok!(a.open_dir("").await);
    let file = ok!(again.open_file("z.txt").await);
    check_eq!(ok!(file.read_to_end().await), Vec::<u8>::new());
    Verdict::Pass
}

pub(crate) async fn reports_open_errors<D: Driver>(ctx: &Ctx<D>) -> Verdict {
    let root = &ctx.root;
    let error = expect_error!(root.open_file("a/nope").await, ErrorKind::NotFound);
    check_eq!(error.path().map(|p| p.as_str()), Some("a/nope"));
    expect_error!(root.open_file("a").await, ErrorKind::IsADirectory);
    expect_error!(root.open_dir("a/z.txt").await, ErrorKind::NotADirectory);
    expect_error!(root.open_file("a/z.txt/x").await, ErrorKind::NotADirectory);
    expect_error!(root.open_file("../a/z.txt").await, ErrorKind::InvalidName);
    Verdict::Pass
}

pub(crate) async fn follows_symlinks_beneath<D: Driver>(ctx: &Ctx<D>) -> Verdict {
    need_symlinks!(ctx);
    let root = &ctx.root;
    for path in ["a/link", "a/up"] {
        let file = ok!(root.open_file(path).await);
        check_eq!(ok!(file.read_to_end().await), b"contents".to_vec());
    }
    let a = ok!(root.open_dir("a").await);
    // `up` leaves `a` before re-entering it: fine from the root, an escape
    // from `a`.
    expect_error!(a.open_file("up").await, ErrorKind::EscapeBlocked);
    expect_error!(root.open_file("a/abs").await, ErrorKind::EscapeBlocked);
    expect_error!(root.open_file("a/escape").await, ErrorKind::EscapeBlocked);
    Verdict::Pass
}

pub(crate) async fn treats_the_handle_as_root<D: Driver>(ctx: &Ctx<D>) -> Verdict {
    need_symlinks!(ctx);
    let root = ctx.root.clone().with_resolve(Symlinks::InRoot);
    let file = ok!(root.open_file("a/abs").await);
    check_eq!(ok!(file.read_to_end().await), b"contents".to_vec());
    // `../../c.txt` clamps to the root, which has no `c.txt`.
    expect_error!(root.open_file("a/escape").await, ErrorKind::NotFound);
    Verdict::Pass
}

pub(crate) async fn denies_symlinks<D: Driver>(ctx: &Ctx<D>) -> Verdict {
    need_symlinks!(ctx);
    let root = ctx.root.clone().with_resolve(Symlinks::Deny);
    expect_error!(root.open_file("a/link").await, ErrorKind::SymlinkLoop);
    expect_error!(
        root.metadata("a/link", MetaMask::KIND).await,
        ErrorKind::SymlinkLoop
    );
    ok!(root.open_file("a/b/c.txt").await);
    ok!(root.symlink_metadata("a/link", MetaMask::KIND).await);
    Verdict::Pass
}

pub(crate) async fn stops_loops_and_dangling_links<D: Driver>(ctx: &Ctx<D>) -> Verdict {
    need_symlinks!(ctx);
    expect_error!(ctx.root.open_file("a/loop").await, ErrorKind::SymlinkLoop);
    expect_error!(ctx.root.open_file("a/dangling").await, ErrorKind::NotFound);
    Verdict::Pass
}

pub(crate) async fn reads_metadata<D: Driver>(ctx: &Ctx<D>) -> Verdict {
    let root = &ctx.root;
    let dir = ok!(root.metadata("a", MetaMask::all()).await);
    check_eq!(dir.kind, Some(FileKind::Dir));
    let file = ok!(root.metadata("a/b/c.txt", MetaMask::all()).await);
    check_eq!((file.kind, file.len), (Some(FileKind::File), Some(8)));
    let opened = ok!(root.open_file("a/b/c.txt").await);
    let by_handle = ok!(opened.metadata(MetaMask::all()).await);
    check_eq!(by_handle.id, file.id);
    check!(
        file.id != dir.id || file.id.is_none(),
        "file and directory share an id"
    );
    if ctx.profile.symlinks {
        let link = ok!(root.symlink_metadata("a/link", MetaMask::all()).await);
        check_eq!((link.kind, link.len), (Some(FileKind::Symlink), Some(7)));
        let target = ok!(root.metadata("a/link", MetaMask::all()).await);
        check_eq!(target.kind, Some(FileKind::File));
    }
    let error = expect_error!(
        root.metadata("a/nope", MetaMask::KIND).await,
        ErrorKind::NotFound
    );
    check_eq!(error.path().map(|p| p.as_str()), Some("a/nope"));
    Verdict::Pass
}

pub(crate) async fn reads_xattrs<D: Driver>(ctx: &Ctx<D>) -> Verdict {
    let file = ok!(ctx.root.open_file("a/b/c.txt").await);
    if !ctx.profile.xattrs {
        // A driver without extended attributes has none, or says so.
        for result in [
            file.xattr_names().await.map(|names| names.is_empty()),
            file.xattr("user.comment")
                .await
                .map(|value| value.is_none()),
        ] {
            match result {
                Ok(true) => {},
                Ok(false) => return Verdict::Fail(String::from("found unexpected attributes")),
                Err(error) if error.kind() == ErrorKind::Unsupported => {},
                Err(error) => return Verdict::Fail(format!("unexpected error: {error}")),
            }
        }
        return Verdict::Skip("no xattrs in this profile");
    }
    let mut names: Vec<String> = ok!(file.xattr_names().await)
        .iter()
        .map(ToString::to_string)
        .collect();
    names.sort();
    check_eq!(names, ["user.comment", "user.empty"]);
    check_eq!(
        ok!(file.xattr("user.comment").await).as_deref(),
        Some(&b"a comment"[..])
    );
    check_eq!(
        ok!(file.xattr("user.empty").await).as_deref(),
        Some(&b""[..])
    );
    check_eq!(ok!(file.xattr("user.nope").await), None);
    // A short buffer gets the start of the value, and its whole length.
    let filer_api::BufResult(len, buf) =
        file.xattr_into("user.comment", Vec::with_capacity(3)).await;
    check_eq!((ok!(len), buf.as_slice()), (Some(9), &b"a c"[..]));
    expect_error!(file.xattr("comment").await, ErrorKind::InvalidName);
    let dir = ok!(ctx.root.open_dir("a").await);
    check_eq!(
        ok!(dir.xattr("user.tag").await).as_deref(),
        Some(&b"dir"[..])
    );
    let names = ok!(dir.xattr_names().await);
    check_eq!(names.len(), 1);
    let blind = dir.restrict(Rights::all() - Rights::READ);
    expect_error!(blind.xattr_names().await, ErrorKind::RightsDenied);
    expect_error!(blind.xattr("user.tag").await, ErrorKind::RightsDenied);
    Verdict::Pass
}

pub(crate) async fn reads_links<D: Driver>(ctx: &Ctx<D>) -> Verdict {
    need_symlinks!(ctx);
    let root = &ctx.root;
    check_eq!(
        ok!(root.read_link("a/abs").await).as_str(),
        Some("/a/b/c.txt")
    );
    check_eq!(
        ok!(root.read_link("a/dangling").await).as_str(),
        Some("nope")
    );
    expect_error!(root.read_link("a/z.txt").await, ErrorKind::NotASymlink);
    expect_error!(root.read_link("").await, ErrorKind::InvalidName);
    Verdict::Pass
}

pub(crate) async fn lists_directories<D: Driver>(ctx: &Ctx<D>) -> Verdict {
    let a = ok!(ctx.root.open_dir("a").await);
    let mut expected = alloc::vec![
        ("b", Some(FileKind::Dir)),
        ("empty", Some(FileKind::Dir)),
        ("z.txt", Some(FileKind::File)),
    ];
    if ctx.profile.symlinks {
        for name in ["abs", "dangling", "escape", "link", "loop", "up"] {
            expected.push((name, Some(FileKind::Symlink)));
        }
    }
    expected.sort_by_key(|(name, _)| *name);
    let listed = ok!(list(&a).await);
    let names: Vec<&str> = listed.iter().map(|(name, _)| name.as_str()).collect();
    let expected_names: Vec<&str> = expected.iter().map(|(name, _)| *name).collect();
    check_eq!(names, expected_names);
    for ((name, kind), (_, expected)) in listed.iter().zip(&expected) {
        // A listing may not know an entry's kind, but must not misreport it.
        check!(
            kind.is_none() || kind == expected,
            "{name} listed as {kind:?}"
        );
    }
    let empty = ok!(a.open_dir("empty").await);
    check!(
        ok!(list(&empty).await).is_empty(),
        "empty directory lists entries"
    );
    Verdict::Pass
}

pub(crate) async fn lists_large_directories<D: Driver>(ctx: &Ctx<D>) -> Verdict {
    let many = ok!(ctx.root.open_dir("many").await);
    let listed = ok!(list(&many).await);
    let names: Vec<String> = listed.into_iter().map(|(name, _)| name).collect();
    let expected: Vec<String> = (0..MANY).map(|i| format!("f{i:03}")).collect();
    check_eq!(names.len(), expected.len());
    check!(
        names == expected,
        "listing of `many` lost or repeated entries"
    );
    Verdict::Pass
}

pub(crate) async fn enforces_rights<D: Driver>(ctx: &Ctx<D>) -> Verdict {
    let blind = ctx.root.restrict(Rights::all() - Rights::READ);
    expect_error!(blind.open_file("a/b/c.txt").await, ErrorKind::RightsDenied);
    expect_error!(
        blind.metadata("a", MetaMask::KIND).await,
        ErrorKind::RightsDenied
    );
    expect_error!(blind.entries(), ErrorKind::RightsDenied);
    let child = ok!(blind.open_dir("a").await);
    check_eq!(child.rights(), blind.rights());
    expect_error!(child.open_file("z.txt").await, ErrorKind::RightsDenied);
    Verdict::Pass
}

pub(crate) async fn rejects_other_drivers_handles<D: Driver>(ctx: &Ctx<D>) -> Verdict {
    check!(
        ctx.root.driver().id() != ctx.other.driver().id(),
        "two instances share an id"
    );
    let op = Op::Open {
        at: ctx.other.handle().clone(),
        path: "a".try_into().expect("valid path"),
        how: filer_api::OpenHow::dir(),
    };
    let Completion { result, .. } = ctx.root.driver().submit(op).await;
    expect_error!(result, ErrorKind::CrossDriver);
    expect_error!(
        Dir::new(ctx.root.driver().clone(), ctx.other.handle().clone()),
        ErrorKind::CrossDriver
    );
    Verdict::Pass
}

pub(crate) async fn refuses_fifos<D: Driver>(ctx: &Ctx<D>) -> Verdict {
    if !ctx.profile.fifos {
        return Verdict::Skip("no FIFOs in this profile");
    }
    // Must fail promptly rather than block waiting for a writer.
    expect_error!(
        ctx.root.open_file("special/fifo").await,
        ErrorKind::NotRegularFile
    );
    Verdict::Pass
}

pub(crate) async fn lists_and_stats_foreign_names<D: Driver>(ctx: &Ctx<D>) -> Verdict {
    if !ctx.profile.foreign_names {
        return Verdict::Skip("no foreign names in this profile");
    }
    let dir = ok!(ctx.root.open_dir("foreign").await);
    let mut entries = ok!(dir.entries());
    let Some(entry) = entries.next().await else {
        return Verdict::Fail(String::from("the foreign name was not listed"));
    };
    let entry = ok!(entry);
    let EntryName::Foreign(name) = &entry.name else {
        return Verdict::Fail(format!("{} was listed as UTF-8", entry.name));
    };
    check_eq!(name.as_bytes(), FOREIGN_NAME);
    check_eq!(name.origin(), ctx.root.driver().id());
    check_eq!(entry.name.to_string(), "caf\\xE9");
    let stat = |at, name| Op::Stat {
        target: Target::Entry {
            at,
            name,
            follow: false,
            resolve: Resolve::default(),
        },
        mask: MetaMask::all(),
    };
    let op = stat(dir.handle().clone(), entry.name.clone());
    match ctx.root.driver().submit(op).await.result {
        Ok(Output::Metadata(metadata)) => check_eq!(metadata.len, Some(7)),
        Ok(output) => return Verdict::Fail(format!("stat returned {output:?}")),
        Err(error) => return Verdict::Fail(format!("stat failed: {error}")),
    }
    // The same name minted by another driver instance is refused.
    let other = ok!(ctx.other.open_dir("foreign").await);
    let op = stat(other.handle().clone(), entry.name);
    let Completion { result, .. } = ctx.other.driver().submit(op).await;
    expect_error!(result, ErrorKind::ForeignNameMismatch);
    Verdict::Pass
}
