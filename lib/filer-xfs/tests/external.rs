// This is free and unencumbered software released into the public domain.

//! Volumes of other projects' test suites, which `tests/external.sh`
//! fetches, checks by their SHA-256 sums, and unpacks into
//! `$FILER_XFS_EXTERNAL`; ignored unless run so, as they are not public
//! domain, nor committed here. Each is walked as the fuzzer walks its
//! inputs, or refused as expected; and the volumes whose logs need
//! recovery are compared with the trees their makers recorded of them, as
//! Linux recovers them.

mod common;

use common::exercise::exercise;
use filer_api::{ErrorKind, FileKind, MetaMask};
use filer_spec::block_on;
use filer_system::{Dir, ReadAt, ambient_authority};
use filer_xfs::Xfs;
use std::path::PathBuf;

/// What a volume should do.
enum Expect {
    /// Open, and walk with `errors` refusals and corruptions found.
    Walks { errors: usize },
    /// Be refused with this kind of error.
    Refused(ErrorKind),
    /// Open and walk, with any errors: what this driver does not yet read
    /// as Linux does.
    Known,
}

use Expect::{Known, Refused, Walks};

/// Every volume, by its file's name in `images`.
const VOLUMES: &[(&str, Expect)] = &[
    // go-xfs-filesystem.
    ("dirblk8k.xfs", Walks { errors: 0 }),
    ("image.xfs", Walks { errors: 0 }),
    ("image40.xfs", Walks { errors: 0 }),
    // lamxfs: samples of superblocks only, a zoned volume, and logs.
    ("v4_asciici_reject.img", Refused(ErrorKind::Corrupt)),
    ("v4_dirty_log.img", Walks { errors: 0 }),
    ("v4_legacy.img", Walks { errors: 0 }),
    ("v5_asciici_reject.img", Refused(ErrorKind::Corrupt)),
    ("v5_bs64k.img", Walks { errors: 0 }),
    ("v5_dirty_churn.img", Walks { errors: 0 }),
    ("v5_dirty_idle.img", Walks { errors: 0 }),
    ("v5_dirty_kernel_update.img", Walks { errors: 0 }),
    ("v5_dirty_log.img", Walks { errors: 0 }),
    ("v5_dirty_parent.img", Walks { errors: 0 }),
    // A write torn at the log's head, which v5 record CRCs tell (XFS-15).
    ("v5_dirty_torn.img", Known),
    ("v5_dirty_wrapped.img", Walks { errors: 0 }),
    ("v5_lts618.img", Walks { errors: 0 }),
    ("v5_metadir.img", Walks { errors: 0 }),
    ("v5_modern.img", Walks { errors: 0 }),
    ("v5_nonrext64.img", Walks { errors: 0 }),
    ("v5_nrext64_upgraded.img", Walks { errors: 0 }),
    ("v5_rtgroups_reject.img", Refused(ErrorKind::Corrupt)),
    ("v5_sect8k.img", Walks { errors: 0 }),
    ("v5_wrapped_log.img", Walks { errors: 0 }),
    ("v5_zoned_reject.img", Refused(ErrorKind::Unsupported)),
    // xfuse, with a realtime file, whose contents are not read.
    ("xfs1024.img", Walks { errors: 0 }),
    ("xfs4096.img", Walks { errors: 0 }),
    ("xfs_4kn.img", Walks { errors: 0 }),
    ("xfs_noftype.img", Walks { errors: 0 }),
    ("xfs_nrext64.img", Walks { errors: 0 }),
    ("xfs_preallocated.img", Walks { errors: 0 }),
    ("xfs_rt1.img", Walks { errors: 2 }),
    ("xfs_xattr_v1.img", Walks { errors: 0 }),
    ("xfsv4.img", Walks { errors: 0 }),
];

/// The volumes whose trees lamxfs recorded, as Linux recovers them.
const TREES: &[&str] = &[
    "v5_dirty_churn",
    "v5_dirty_idle",
    "v5_dirty_kernel_update",
    "v5_dirty_log",
    "v5_dirty_parent",
    "v5_dirty_wrapped",
];

/// Returns where `tests/external.sh` put what it fetched.
fn fetched() -> PathBuf {
    let dir =
        std::env::var_os("FILER_XFS_EXTERNAL").expect("FILER_XFS_EXTERNAL: run tests/external.sh");
    PathBuf::from(dir)
}

/// Opens `name` in directory `dir`, as a file of the native driver.
fn open(dir: &std::path::Path, name: &str) -> impl ReadAt + 'static {
    let dir = filer_system::open_ambient(dir, ambient_authority()).unwrap();
    block_on(dir.open_file(name)).unwrap()
}

#[test]
#[ignore = "needs tests/external.sh"]
fn reads_other_projects_volumes() {
    let images = fetched().join("images");
    for (name, expect) in VOLUMES {
        let file = open(&images, name);
        match expect {
            Walks { errors } => {
                let outcome = exercise(file);
                assert!(outcome.opened, "{name}");
                assert_eq!(outcome.errors, *errors, "{name}: {outcome:?}");
            },
            Refused(kind) => {
                let error = block_on(Xfs::open(file)).unwrap_err();
                assert_eq!(error.kind(), *kind, "{name}: {error}");
            },
            Known => {
                exercise(file);
            },
        }
    }
}

#[test]
#[ignore = "needs tests/external.sh"]
fn replays_logs_as_recorded() {
    let dir = fetched();
    let trees = dir.join("lamxfs/tests/fixtures");
    for name in TREES {
        let recorded = std::fs::read_to_string(trees.join(format!("{name}.tree"))).unwrap();
        let mut recorded: Vec<&str> = recorded.lines().collect();
        recorded.sort_unstable();
        let root = block_on(Xfs::open(open(&dir.join("images"), &format!("{name}.img")))).unwrap();
        let mut found = Vec::new();
        tree(&root, "", &mut found);
        found.sort_unstable();
        assert_eq!(found, recorded, "{name}");
    }
}

/// Lists everything below `dir`, a path from the root's `/`, or the root
/// itself if empty, as lamxfs's trees do: a directory as `d`
/// and its path, a file as `f`, its path, its size, and the SHA-256 of
/// its contents, a symlink as `l`, its path, and its target, and anything
/// else as `o` and its path.
fn tree<S: ReadAt + 'static>(root: &Dir<Xfs<S>>, dir: &str, out: &mut Vec<String>) {
    let handle = block_on(root.open_dir(dir.trim_start_matches('/'))).unwrap();
    let mut entries = handle.entries().unwrap();
    while let Some(entry) = block_on(entries.next()) {
        let entry = entry.unwrap();
        let path = format!("{dir}/{}", entry.name);
        let relative = path.trim_start_matches('/');
        let metadata = block_on(root.symlink_metadata(relative, MetaMask::all())).unwrap();
        match metadata.kind {
            Some(FileKind::Dir) => {
                out.push(format!("d {path}"));
                tree(root, &path, out);
            },
            Some(FileKind::File) => {
                let file = block_on(root.open_file(relative)).unwrap();
                let contents = block_on(file.read_to_end()).unwrap();
                out.push(format!("f {path} {} {}", contents.len(), sha256(&contents)));
            },
            Some(FileKind::Symlink) => {
                let target = block_on(root.read_link(relative)).unwrap();
                out.push(format!(
                    "l {path} {}",
                    String::from_utf8_lossy(target.as_bytes())
                ));
            },
            _ => out.push(format!("o {path}")),
        }
    }
}

/// Returns the SHA-256 of `bytes`, in hexadecimal.
fn sha256(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = bytes.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend((bytes.len() as u64 * 8).to_be_bytes());
    for block in message.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes(word.try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            (hh, g, f, e, d, c, b, a) = (g, f, e, d.wrapping_add(t1), c, b, a, t1.wrapping_add(t2));
        }
        for (state, add) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *state = state.wrapping_add(add);
        }
    }
    h.iter().map(|word| format!("{word:08x}")).collect()
}

#[test]
fn hashes_as_sha256() {
    assert_eq!(
        sha256(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let long = vec![b'a'; 1000];
    assert_eq!(
        sha256(&long),
        "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3"
    );
}
