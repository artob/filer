// This is free and unencumbered software released into the public domain.

//! Valid volumes for the targets to lay their input over, so that the
//! fuzzer starts past the boot sector's checks, and where their
//! structures are.

// Each target uses only part of it.
#![allow(dead_code)]

use crate::common::{exercise::Overlay, unsparse};
use std::sync::{Arc, OnceLock};

/// A template volume, and the offsets of its structures.
pub struct Template {
    pub image: Arc<Vec<u8>>,
    /// The MFT's first record.
    pub mft: u64,
    /// The first index record.
    pub index: u64,
    /// The first cluster holding a compressed LZNT1 chunk, if any.
    pub compressed: u64,
}

/// The volume of `make.sh`'s `fill`, with 4 KiB clusters.
pub fn plain() -> &'static Template {
    static TEMPLATE: OnceLock<Template> = OnceLock::new();
    TEMPLATE.get_or_init(|| build(include_bytes!("../../tests/fixtures/ntfs-4k.sparse")))
}

/// The volume of compressed files.
pub fn compressed() -> &'static Template {
    static TEMPLATE: OnceLock<Template> = OnceLock::new();
    TEMPLATE.get_or_init(|| {
        build(include_bytes!(
            "../../tests/fixtures/ntfs-compressed.sparse"
        ))
    })
}

fn build(sparse: &[u8]) -> Template {
    let image = unsparse(sparse);
    let sector = u64::from(u16::from_le_bytes([image[11], image[12]]));
    let cluster = sector * u64::from(image[13]);
    let mft = u64::from_le_bytes(image[0x30..0x38].try_into().unwrap()) * cluster;
    let find = |found: &dyn Fn(&[u8]) -> bool| {
        (0..image.len() as u64)
            .step_by(cluster as usize)
            .skip(1)
            .find(|&at| found(&image[at as usize..]))
            .unwrap_or(0)
    };
    let index = find(&|bytes| bytes.starts_with(b"INDX"));
    // A compressed chunk's header has its top bit set and signature 3.
    let compressed = find(&|bytes| bytes[1] & 0xF0 == 0xB0 && bytes[2] != 0);
    Template {
        image: Arc::new(image),
        mft,
        index,
        compressed,
    }
}

/// Lays `data` over `template` at `at`.
pub fn overlay(template: &Template, at: u64, data: &[u8]) -> Overlay {
    let mut overlay = Overlay::new(template.image.clone());
    overlay.patch(at, data);
    overlay
}
