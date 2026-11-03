// This is free and unencumbered software released into the public domain.

//! Whole archives: the input is the archive. Seed the corpus with real
//! ones, such as those in `tests/data`.

#![no_main]

use filer_cpio::Cpio;
use filer_spec::{Checks, block_on, exercise};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let checks = Checks::new().kinds(true).xattrs(false).refusals(true);
    exercise(block_on(Cpio::open(data.to_vec())), checks);
});
