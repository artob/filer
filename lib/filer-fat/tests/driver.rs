// This is free and unencumbered software released into the public domain.

//! The driver over written images: reading, lookup, listing, metadata,
//! and corrupt volumes.

mod common;

use common::{Image, KINDS, Kind, MTIME, dir, file};
use filer_api::{Dir, Error, ErrorKind, FileKind, MetaMask};
use filer_fat::{Fat, FatError};
use filer_spec::block_on;

type Root = Dir<Fat<Vec<u8>>>;

fn open(image: Image) -> Root {
    block_on(Fat::open(image.bytes)).unwrap()
}

fn read(root: &Root, path: &str) -> Result<Vec<u8>, Error> {
    let file = block_on(root.open_file(path))?;
    block_on(file.read_to_end())
}

fn list(root: &Root, path: &str) -> Vec<String> {
    let dir = block_on(root.open_dir(path)).unwrap();
    let mut entries = dir.entries().unwrap();
    let mut names = Vec::new();
    while let Some(entry) = block_on(entries.next()) {
        names.push(entry.unwrap().name.to_string());
    }
    names
}

fn fat_error(error: &Error) -> FatError {
    *core::error::Error::source(error)
        .and_then(|source| source.downcast_ref::<FatError>())
        .unwrap_or_else(|| panic!("no FAT error in {error:?}"))
}

/// Returns `len` bytes of a recognizable pattern.
fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 7 + i / 251) as u8).collect()
}

fn sample(kind: Kind, fragmented: bool) -> Image {
    let tree = vec![
        file("README.TXT", "Read me."),
        file("readme2.txt", "Lowercase."),
        file("A long file name.txt", "Long."),
        file("empty", ""),
        dir(
            "Sub",
            vec![
                file("nested.bin", pattern(5000)),
                dir("Deeper", vec![file("leaf", "Leaf.")]),
            ],
        ),
    ];
    if fragmented {
        Image::fragmented(kind, tree)
    } else {
        Image::new(kind, tree)
    }
}

#[test]
fn reads_every_fat_type() {
    for kind in KINDS {
        for fragmented in [false, true] {
            let root = open(sample(kind, fragmented));
            let what = format!("{kind:?}, fragmented: {fragmented}");
            assert_eq!(read(&root, "README.TXT").unwrap(), b"Read me.", "{what}");
            assert_eq!(read(&root, "readme2.txt").unwrap(), b"Lowercase.", "{what}");
            assert_eq!(
                read(&root, "A long file name.txt").unwrap(),
                b"Long.",
                "{what}"
            );
            assert_eq!(read(&root, "empty").unwrap(), b"", "{what}");
            assert_eq!(
                read(&root, "Sub/nested.bin").unwrap(),
                pattern(5000),
                "{what}"
            );
            assert_eq!(read(&root, "Sub/Deeper/leaf").unwrap(), b"Leaf.", "{what}");
        }
    }
}

#[test]
fn reads_at_any_offset() {
    let data = pattern(5000);
    for kind in KINDS {
        let root = open(sample(kind, true));
        let file = block_on(root.open_file("Sub/nested.bin")).unwrap();
        for (off, len) in [
            (0, 1),
            (511, 2),
            (512, 512),
            (1000, 3000),
            (4999, 10),
            (5000, 1),
        ] {
            let buf = Vec::with_capacity(len);
            let filer_api::BufResult(result, buf) = block_on(file.read_at(buf, off as u64));
            let n = result.unwrap();
            let end = (off + len).min(data.len());
            // A read stops at the end of a run of clusters, or of the file.
            assert!(n > 0 || off == data.len(), "{kind:?} at {off}");
            assert_eq!(buf, data[off..off + n], "{kind:?} at {off}");
            assert!(off + n <= end);
        }
    }
}

#[test]
fn lists_directories_in_order() {
    for kind in KINDS {
        let root = open(sample(kind, false));
        assert_eq!(
            list(&root, ""),
            [
                "README.TXT",
                "readme2.txt",
                "A long file name.txt",
                "empty",
                "Sub"
            ]
        );
        assert_eq!(list(&root, "Sub"), ["nested.bin", "Deeper"]);
    }
}

#[test]
fn lists_large_directories_across_batches() {
    let names: Vec<&'static str> = (0..300)
        .map(|i| &*Box::leak(format!("File number {i}").into_boxed_str()))
        .collect();
    for kind in [Kind::Fat16, Kind::Fat32] {
        let children = names.iter().map(|&name| file(name, "")).collect();
        let root = open(Image::new(kind, vec![dir("Many", children)]));
        assert_eq!(list(&root, "Many"), names, "{kind:?}");
    }
}

#[test]
fn looks_names_up_ignoring_case() {
    for kind in KINDS {
        let root = open(sample(kind, false));
        for path in [
            "readme.txt",
            "ReadMe.Txt",
            "README2.TXT",
            "a LONG file NAME.TXT",
            "sub/NESTED.BIN",
            "SUB/deeper/LEAF",
        ] {
            assert!(read(&root, path).is_ok(), "{kind:?}: {path}");
        }
        // Short aliases of long names.
        assert_eq!(read(&root, "ALONGF~1.TXT").unwrap(), b"Long.");
        assert_eq!(read(&root, "sub~1/nested.bin").unwrap(), pattern(5000));
        assert_eq!(
            block_on(root.open_file("README.TX")).unwrap_err().kind(),
            ErrorKind::NotFound
        );
    }
}

#[test]
fn opens_relative_to_subdirectories() {
    let root = open(sample(Kind::Fat16, false));
    let sub = block_on(root.open_dir("Sub")).unwrap();
    assert_eq!(read(&sub, "nested.bin").unwrap(), pattern(5000));
    let deeper = block_on(sub.open_dir("deeper")).unwrap();
    assert_eq!(read(&deeper, "LEAF").unwrap(), b"Leaf.");
    assert_eq!(list(&deeper, ""), ["leaf"]);
}

#[test]
fn reports_kinds_and_metadata() {
    for kind in KINDS {
        let root = open(sample(kind, false));
        let all = MetaMask::all();
        let file = block_on(root.metadata("Sub/nested.bin", all)).unwrap();
        assert_eq!(file.kind, Some(FileKind::File));
        assert_eq!(file.len, Some(5000));
        let mtime = file.modified.unwrap();
        assert_eq!(
            (mtime.secs(), mtime.granularity()),
            (MTIME, Some(core::time::Duration::from_secs(2)))
        );
        assert_eq!(file.created.unwrap().secs(), MTIME);
        assert_eq!(file.accessed.unwrap().secs(), MTIME - 3 * 3600 - 4 * 60 - 6);

        let sub = block_on(root.metadata("Sub", all)).unwrap();
        assert_eq!(sub.kind, Some(FileKind::Dir));
        let top = block_on(root.metadata("", all)).unwrap();
        assert_eq!(top.kind, Some(FileKind::Dir));

        // Ids are distinct, and the same through a handle as by path.
        let handle = block_on(root.open_file("Sub/nested.bin")).unwrap();
        let through_handle = block_on(handle.metadata(all)).unwrap();
        assert_eq!(through_handle.id, file.id);
        let empty = block_on(root.metadata("empty", all)).unwrap();
        let ids = [file.id, sub.id, top.id, empty.id];
        for (i, a) in ids.iter().enumerate() {
            assert!(a.is_some());
            assert!(ids[i + 1..].iter().all(|b| b != a), "{kind:?}");
        }
    }
}

#[test]
fn refuses_operations_on_the_wrong_kind() {
    let root = open(sample(Kind::Fat12, false));
    let not_a_dir = block_on(root.open_dir("README.TXT")).unwrap_err();
    assert_eq!(not_a_dir.kind(), ErrorKind::NotADirectory);
    let is_a_dir = block_on(root.open_file("Sub")).unwrap_err();
    assert_eq!(is_a_dir.kind(), ErrorKind::IsADirectory);
    let not_a_link = block_on(root.read_link("README.TXT")).unwrap_err();
    assert_eq!(not_a_link.kind(), ErrorKind::NotASymlink);
    let through = block_on(root.open_file("README.TXT/x")).unwrap_err();
    assert_eq!(through.kind(), ErrorKind::NotADirectory);
}

#[test]
fn refuses_corrupt_chains() {
    for kind in KINDS {
        // A file whose chain ends before its size does.
        let mut image = sample(kind, false);
        let first = image.clusters["Sub/nested.bin"];
        image.set_fat(first, 0x0FFF_FFFF);
        let root = open(image);
        let error = read(&root, "Sub/nested.bin").unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt);
        assert!(
            matches!(fat_error(&error), FatError::ShortChain { .. }),
            "{kind:?}"
        );
        // The rest of the volume still reads.
        assert_eq!(read(&root, "README.TXT").unwrap(), b"Read me.");

        // A directory whose chain loops.
        let mut image = sample(kind, false);
        let sub = image.clusters["Sub"];
        image.set_fat(sub, sub);
        let root = open(image);
        let error = block_on(root.open_dir("Sub")).unwrap_err();
        // Caught by whichever bound comes first: the volume's clusters, or
        // the most a directory may have.
        assert!(matches!(
            fat_error(&error),
            FatError::ChainLoop { start } | FatError::DirectoryTooLarge { start }
                if start == sub
        ));
    }
}

#[test]
fn refuses_corrupt_entries() {
    let mut image = sample(Kind::Fat16, false);
    image.slot("README.TXT")[0] = b'/';
    let root = open(image);
    let dir = block_on(root.open_dir("")).unwrap();
    let mut entries = dir.entries().unwrap();
    let error = loop {
        match block_on(entries.next()) {
            Some(Ok(_)) => {},
            Some(Err(error)) => break error,
            None => panic!("no error"),
        }
    };
    assert!(matches!(fat_error(&error), FatError::BadName { .. }));

    let mut image = sample(Kind::Fat16, false);
    image.slot("empty")[26] = 7; // An empty file with a cluster.
    let root = open(image);
    // Lookups before the entry succeed; those that pass it fail.
    assert_eq!(read(&root, "README.TXT").unwrap(), b"Read me.");
    let error = read(&root, "Sub/nested.bin").unwrap_err();
    assert!(matches!(fat_error(&error), FatError::StrayCluster { .. }));
}

#[test]
fn refuses_truncated_volumes() {
    let mut image = sample(Kind::Fat12, false);
    image.bytes.truncate(1_000_000);
    let error = block_on(Fat::open(image.bytes)).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Corrupt);
    assert_eq!(
        fat_error(&error),
        FatError::Truncated {
            volume: 1_474_560,
            source: 1_000_000
        }
    );
}

/// Writes this file's sample images to the directory named by
/// `FILER_FAT_DUMP`, for `tests/differential.sh` to compare with Linux.
#[test]
#[ignore = "run by tests/differential.sh"]
fn dump_images() {
    let dir = std::env::var_os("FILER_FAT_DUMP").expect("FILER_FAT_DUMP");
    let dir = std::path::Path::new(&dir);
    for kind in KINDS {
        for fragmented in [false, true] {
            let image = sample(kind, fragmented);
            let name = format!(
                "writer-{kind:?}{}.img",
                if fragmented { "-fragmented" } else { "" }
            );
            std::fs::write(dir.join(name.to_lowercase()), image.bytes).unwrap();
        }
    }
}

#[test]
fn reads_times_at_the_given_offset() {
    use filer_fat::{Options, UtcOffset};
    let all = MetaMask::all();
    let utc = block_on(open(sample(Kind::Fat16, false)).metadata("README.TXT", all)).unwrap();
    for minutes in [-300, 90, UtcOffset::MAX_MINUTES] {
        let offset = UtcOffset::from_minutes(minutes).unwrap();
        let options = Options::new().with_utc_offset(offset);
        let image = sample(Kind::Fat16, false).bytes;
        let root = block_on(Fat::open_with(image, options)).unwrap();
        let local = block_on(root.metadata("README.TXT", all)).unwrap();
        let shift = i64::from(minutes) * 60;
        for (utc, local) in [
            (utc.modified, local.modified),
            (utc.created, local.created),
            (utc.accessed, local.accessed),
        ] {
            assert_eq!(local.unwrap().secs(), utc.unwrap().secs() - shift);
        }
    }
}

#[cfg(feature = "code-pages")]
#[test]
fn decodes_short_names_in_the_given_code_page() {
    use filer_fat::{CodePage, Options};
    // "R\x9BADME.TXT" has no long name; 0x9B is ¢ in code page 437, ø in
    // 850, and undefined in none of these.
    let mut image = sample(Kind::Fat16, false);
    image.slot("README.TXT")[1] = 0x9B;
    let open_in = |image: &Image, page| {
        let options = Options::new().with_code_page(page);
        block_on(Fat::open_with(image.bytes.clone(), options)).unwrap()
    };
    assert_eq!(
        read(&open_in(&image, CodePage::Cp437), "R¢ADME.TXT").unwrap(),
        b"Read me."
    );
    assert_eq!(
        read(&open_in(&image, CodePage::Cp850), "RøADME.TXT").unwrap(),
        b"Read me."
    );
    assert_eq!(
        read(&open_in(&image, CodePage::Cp850), "RØADME.txt").unwrap(),
        b"Read me."
    );
    assert!(list(&open_in(&image, CodePage::Cp866), "").contains(&"RЫADME.TXT".to_owned()));

    // 0xD5 is undefined in code page 857: listing the entry fails.
    image.slot("README.TXT")[1] = 0xD5;
    let root = open_in(&image, CodePage::Cp857);
    let dir = block_on(root.open_dir("")).unwrap();
    let mut entries = dir.entries().unwrap();
    let error = block_on(entries.next()).unwrap().unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Corrupt);
    assert!(matches!(
        fat_error(&error),
        FatError::Undecodable {
            byte: 0xD5,
            code_page: 857,
            ..
        }
    ));
}

#[test]
fn reports_directory_lengths() {
    for (kind, root_len, sub_len) in [
        // The fixed roots of FAT12 and FAT16; one cluster for FAT32's.
        (Kind::Fat12, 224 * 32, 512),
        (Kind::Fat16, 512 * 32, 2048),
        (Kind::Fat32, 512, 512),
    ] {
        let root = open(sample(kind, false));
        let len = |path: &str, mask| block_on(root.metadata(path, mask)).unwrap().len;
        assert_eq!(len("", MetaMask::LEN), Some(root_len), "{kind:?}");
        assert_eq!(len("Sub", MetaMask::LEN), Some(sub_len), "{kind:?}");
        // Without a handle, the chain is walked only when asked for.
        assert_eq!(len("Sub", MetaMask::KIND), None, "{kind:?}");
        let sub = block_on(root.open_dir("Sub")).unwrap();
        let through_handle = block_on(sub.metadata("", MetaMask::KIND)).unwrap();
        assert_eq!(through_handle.len, Some(sub_len), "{kind:?}");
    }
}

#[test]
fn identifies_files_by_volume_serial_number() {
    for kind in KINDS {
        let id = |serial: Option<u32>| {
            let mut image = sample(kind, false);
            let (signature, at) = match kind {
                Kind::Fat12 | Kind::Fat16 => (38, 39),
                Kind::Fat32 => (66, 67),
            };
            match serial {
                Some(serial) => image.bytes[at..at + 4].copy_from_slice(&serial.to_le_bytes()),
                None => image.bytes[signature] = 0,
            }
            let root = open(image);
            block_on(root.metadata("Sub/nested.bin", MetaMask::ID))
                .unwrap()
                .id
                .unwrap()
        };
        let (a, b, none) = (id(Some(1)), id(Some(2)), id(None));
        assert!(a != b && a != none && b != none, "{kind:?}");
        assert_eq!(id(Some(1)), a);
    }
}
