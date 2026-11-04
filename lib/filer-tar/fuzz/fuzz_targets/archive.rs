// This is free and unencumbered software released into the public domain.

//! Whole archives: the input is the archive. Seed the corpus with real
//! ones, such as those in `tests/data`. Each block bearing the `ustar`
//! magic has its checksum resealed first, so that mutations get past it
//! to what parses the header; a header without the magic keeps its own.

#![no_main]

use filer_spec::{Checks, block_on, exercise};
use filer_tar::Tar;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut archive = data.to_vec();
    for header in archive.chunks_exact_mut(512) {
        if header[257..262] == *b"ustar" {
            header[148..156].fill(b' ');
            let sum: u32 = header.iter().map(|&b| u32::from(b)).sum();
            header[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
        }
    }
    let checks = Checks::new().kinds(true).xattrs(false).refusals(true);
    exercise(block_on(Tar::open(archive)), checks);
});
