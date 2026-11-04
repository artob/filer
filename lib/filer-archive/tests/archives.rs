// This is free and unencumbered software released into the public domain.

//! Archives of each format, opened by name, by content, and by format,
//! and the errors of those that cannot be. `tests/data` holds small
//! archives that real tools wrote, copied from each format's crate, but
//! for `hello.cpio.zst`, by zstd 1.5.7, and `hello-sfx.zip`, a stub and
//! `hello.zip` that Info-ZIP's `zip -A` adjusted. Tests of a format this
//! build does not read check that it is refused instead, so they pass
//! with any features.

use filer_api::{ErrorKind, Rights};
use filer_archive::{Archive, Format, FormatError, Limits};
use filer_spec::block_on;

/// Each format's archive, and a file in it with its contents.
const ARCHIVES: [(Format, &str, &str, &[u8]); 4] = [
    (Format::Ar, "hello.a", "notes.txt", b"Hello, world!\n"),
    (
        Format::Cpio,
        "hello.cpio",
        "dir/hello.txt",
        b"Hello, world!\n",
    ),
    (
        Format::Tar,
        "hello.tar",
        "hello/world.txt",
        b"Hello, world!\n",
    ),
    (
        Format::Zip,
        "hello.zip",
        "hello/world.txt",
        b"Hello, world!\n",
    ),
];

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn format_error(error: &filer_api::Error) -> Option<&FormatError> {
    core::error::Error::source(error).and_then(|source| source.downcast_ref())
}

#[test]
fn opens_each_format_by_name() {
    for (format, name, path, contents) in ARCHIVES {
        let opened = block_on(Archive::open(fixture(name), name));
        if !format.is_enabled() {
            let error = opened.unwrap_err();
            assert_eq!(error.kind(), ErrorKind::Unsupported);
            assert_eq!(format_error(&error), Some(&FormatError::Disabled(format)));
            continue;
        }
        let root = opened.unwrap();
        assert_eq!(root.driver().format(), format);
        assert_eq!(root.rights(), Rights::READ);
        let file = block_on(root.open_file(path)).unwrap();
        assert_eq!(block_on(file.read_to_end()).unwrap(), contents, "{name}");
        // The name labels the root, so errors say which archive failed.
        let error = block_on(root.open_file("nope")).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("open \"nope\" in {name}: not found")
        );
    }
}

#[test]
fn opens_compressed_tarballs_by_name() {
    let compressed = [
        ("hello.tar.gz", cfg!(feature = "gzip")),
        ("hello.tar.zst", cfg!(feature = "zstd")),
        ("hello.tar.xz", cfg!(feature = "xz")),
        ("hello.tar.bz2", cfg!(feature = "bzip2")),
    ];
    for (name, enabled) in compressed {
        assert_eq!(Format::from_name(name), Some(Format::Tar));
        let opened = block_on(Archive::open(fixture(name), name));
        if !(enabled && Format::Tar.is_enabled()) {
            assert_eq!(opened.unwrap_err().kind(), ErrorKind::Unsupported);
            continue;
        }
        let root = opened.unwrap();
        let file = block_on(root.open_file("hello/world.txt")).unwrap();
        assert_eq!(block_on(file.read_to_end()).unwrap(), b"Hello, world!\n");
    }
}

#[test]
fn opens_each_format_by_content() {
    // Plain, compressed, and named for another format.
    let mut archives: Vec<(Format, &str, &str)> = ARCHIVES
        .iter()
        .map(|&(format, name, path, _)| (format, name, path))
        .collect();
    for name in [
        "hello.tar.gz",
        "hello.tar.zst",
        "hello.tar.xz",
        "hello.tar.bz2",
    ] {
        archives.push((Format::Tar, name, "hello/world.txt"));
    }
    archives.push((Format::Cpio, "hello.cpio.zst", "dir/hello.txt"));
    // After a self-extractor's stub, its offsets adjusted by `zip -A`.
    archives.push((Format::Zip, "hello-sfx.zip", "hello/world.txt"));
    for (format, name, path) in archives {
        let compression = name
            .rsplit_once('.')
            .and_then(|(_, extension)| match extension {
                "gz" => Some(cfg!(feature = "gzip")),
                "zst" => Some(cfg!(feature = "zstd")),
                "xz" => Some(cfg!(feature = "xz")),
                "bz2" => Some(cfg!(feature = "bzip2")),
                _ => None,
            });
        if !format.is_enabled() || compression == Some(false) {
            continue;
        }
        let root = block_on(Archive::open_by_content(fixture(name))).unwrap();
        assert_eq!(root.driver().format(), format, "{name}");
        let file = block_on(root.open_file(path)).unwrap();
        assert_eq!(block_on(file.read_to_end()).unwrap(), b"Hello, world!\n");
        let misnamed = block_on(Archive::open(fixture(name), "initrd.img.zip")).unwrap();
        assert_eq!(misnamed.driver().format(), format, "{name}");
    }
}

#[test]
fn opens_archives_whose_names_say_nothing() {
    for (format, name, path, contents) in ARCHIVES {
        if !format.is_enabled() {
            continue;
        }
        let root = block_on(Archive::open_as(fixture(name), format)).unwrap();
        assert_eq!(root.label(), None);
        let file = block_on(root.open_file(path)).unwrap();
        assert_eq!(block_on(file.read_to_end()).unwrap(), contents, "{name}");
    }
    // Upper case, as some tools write.
    if Format::Zip.is_enabled() {
        let root = block_on(Archive::open(fixture("hello.zip"), "HELLO.ZIP")).unwrap();
        assert_eq!(root.driver().format(), Format::Zip);
    }
}

#[test]
fn refuses_names_of_no_format() {
    // Content that says nothing either.
    for name in ["hello.txt", "release.gz", "initrd.img", "archive"] {
        let error = block_on(Archive::open(b"Hello, world!".to_vec(), name)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Unsupported, "{name}");
        assert_eq!(
            format_error(&error),
            Some(&FormatError::Unknown(name.into())),
            "{name}"
        );
    }
    let error = FormatError::Unknown("initrd.img".into());
    assert_eq!(
        error.to_string(),
        "no archive format is named by \"initrd.img\", or by its content"
    );
    let error = block_on(Archive::open_by_content(b"Hello!".to_vec())).unwrap_err();
    assert_eq!(format_error(&error), Some(&FormatError::Unrecognized));
    assert_eq!(
        FormatError::Unrecognized.to_string(),
        "no archive format is told by its content"
    );
    let error = FormatError::Disabled(Format::Cpio);
    assert_eq!(error.to_string(), "the cpio format is not enabled");
}

#[test]
fn passes_limits_and_errors_through() {
    for (format, name, ..) in ARCHIVES {
        if !format.is_enabled() {
            continue;
        }
        let limits = Limits::default().with_max_entries(1);
        let error = block_on(Archive::open_with(fixture(name), format, limits)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::LimitExceeded, "{name}");
        // The content outweighs a name that lies about the format, but
        // not a format given.
        let (other, other_format) = if format == Format::Zip {
            ("hello.tar", Format::Tar)
        } else {
            ("hello.zip", Format::Zip)
        };
        if other_format.is_enabled() {
            let root = block_on(Archive::open(fixture(other), name)).unwrap();
            assert_eq!(root.driver().format(), other_format, "{other} as {name}");
        }
        let error = block_on(Archive::open_as(fixture(other), format)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt, "{other} as {name}");
    }
}
