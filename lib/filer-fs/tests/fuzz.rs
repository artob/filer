// This is free and unencumbered software released into the public domain.

//! A structure-aware fuzzer of partition tables: it mutates the fixtures'
//! MBRs, extended boot records, and GPT headers and entries, and reads
//! each result, which must be refused or name only partitions within the
//! disk, each read through a `Window`.
//!
//! Each `cargo test` runs a couple of hundred inputs. For longer runs, set
//! `FILER_FS_FUZZ_ITERATIONS`; `FILER_FS_FUZZ_SEED` picks another
//! sequence, and a failure prints the seed and iteration that reproduce
//! it.

use filer_api::{BufResult, ErrorKind, ReadAt};
use filer_fs::partition::{self, Window};
use filer_spec::{
    Image, Outcome, Overlay, Sparse, block_on,
    fuzz::{Fuzz, Mutations},
};
use std::{ops::Range, sync::Arc};

/// Values at the edges of what partition tables hold.
const BYTES: [u8; 12] = [0, 1, 5, 7, 0x0C, 0x0F, 0x80, 0x83, 0x85, 0xEE, 0xFE, 0xFF];
const WORDS: [u32; 12] = [
    0,
    1,
    2,
    34,
    92,
    128,
    2047,
    2048,
    0x5452_4150,
    0x0001_0000,
    0x7FFF_FFFF,
    0xFFFF_FFFF,
];

/// The mutations to make, with those values.
const MUTATIONS: Mutations = Mutations::new(&BYTES, &WORDS);

/// A disk, and where its tables are: its first 34 sectors of 4 KiB, the
/// sectors around its extended partition's boot records, and its last 64
/// KiB.
fn seed(name: &str) -> (Arc<Sparse>, Vec<Range<u64>>) {
    let path = format!(
        "{}/tests/fixtures/{name}.sparse",
        env!("CARGO_MANIFEST_DIR")
    );
    let disk = Sparse::new(&std::fs::read(path).unwrap());
    let len = disk.length();
    let mut regions = vec![0..512, 512..34 * 4096, len - 64 * 1024..len];
    // `mbr`'s extended boot records, at sectors 67584 and 86016.
    if name == "mbr" {
        regions.push(67584 * 512..67585 * 512);
        regions.push(86016 * 512..86017 * 512);
    }
    (Arc::new(disk), regions)
}

/// Reads the table of `disk`, checking that every partition it names lies
/// within the disk and reads through a window.
fn read(disk: Overlay<Sparse>) -> Outcome {
    let mut outcome = Outcome::default();
    let len = disk.length();
    let table = match block_on(partition::read(&disk)) {
        Ok(Some(table)) => table,
        Ok(None) => return outcome,
        Err(error) => {
            assert_eq!(error.kind(), ErrorKind::Corrupt, "{error:?}");
            outcome.errors = 1;
            return outcome;
        },
    };
    outcome.opened = true;
    for partition in table.partitions() {
        let end = partition.start().checked_add(partition.len()).unwrap();
        assert!(end <= len && partition.start() > 0, "{partition:?}");
        let window = Window::of(disk.clone(), partition);
        let BufResult(result, buf) = block_on(window.read_at(Vec::with_capacity(512), 0));
        let n = result.unwrap();
        assert_eq!(n, buf.len());
        assert_eq!(n, 512.min(usize::try_from(partition.len()).unwrap()));
        outcome.entries += 1;
        outcome.bytes += u64::try_from(n).unwrap();
    }
    outcome
}

#[test]
fn survives_mutated_tables() {
    let corpus: Vec<_> = ["mbr", "gpt", "gpt-4k", "gpt-backup"]
        .into_iter()
        .map(seed)
        .collect();
    Fuzz::new("FILER_FS_FUZZ", 0x9A27_5EED).run(
        corpus.len(),
        |source, rng| {
            let (disk, regions) = &corpus[source];
            MUTATIONS.mutate(disk, regions, rng)
        },
        read,
    );
}
