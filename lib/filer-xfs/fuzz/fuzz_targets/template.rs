// This is free and unencumbered software released into the public domain.

//! Valid volumes for the targets to lay their input over, so that the
//! fuzzer starts past the superblock's checks, and where their structures
//! are.

// Each target uses only part of it.
#![allow(dead_code)]

use crate::common::{Image, Sparse, Overlay};
use std::sync::{Arc, OnceLock};

/// A template volume, and the offsets of its structures.
pub struct Template {
    pub image: Arc<Sparse>,
    /// The superblock.
    pub superblock: u64,
    /// The root directory's inode, and those after it.
    pub inodes: u64,
    /// The first single-block directory.
    pub directory: u64,
    /// The first leaf block of a directory of nodes over leaves.
    pub leaves: u64,
    /// The first `BMAP` B+ tree block.
    pub bmap: u64,
    /// The first extended attribute leaf.
    pub xattrs: u64,
    /// The log.
    pub log: u64,
}

/// The volume of `make.sh`'s `attrs`, with 4 KiB blocks and 256-byte
/// inodes: files, and attribute forks of every form.
pub fn attrs() -> &'static Template {
    static TEMPLATE: OnceLock<Template> = OnceLock::new();
    TEMPLATE.get_or_init(|| build(include_bytes!("../../tests/fixtures/xfs-attrs.sparse")))
}

/// The volume of `make.sh`'s `dirs`: directories of every form.
pub fn dirs() -> &'static Template {
    static TEMPLATE: OnceLock<Template> = OnceLock::new();
    TEMPLATE.get_or_init(|| build(include_bytes!("../../tests/fixtures/xfs-dirs.sparse")))
}

/// The volume of `make.sh`'s `spec_dirty`: its log needs recovery.
pub fn dirty() -> &'static Template {
    static TEMPLATE: OnceLock<Template> = OnceLock::new();
    TEMPLATE.get_or_init(|| build(include_bytes!("../../tests/fixtures/xfs5d-dirty.sparse")))
}

/// The volume of `make.sh`'s `fill`, with 4 KiB blocks: files of every
/// kind, and a single-block directory of 150 entries.
pub fn plain() -> &'static Template {
    static TEMPLATE: OnceLock<Template> = OnceLock::new();
    TEMPLATE.get_or_init(|| build(include_bytes!("../../tests/fixtures/xfs-4k.sparse")))
}

fn build(sparse: &[u8]) -> Template {
    let image = Sparse::new(sparse);
    let be = |at: u64, n: usize| {
        let mut bytes = vec![0; n];
        image.copy(at, &mut bytes);
        bytes
            .iter()
            .fold(0u64, |value, &byte| value << 8 | u64::from(byte))
    };
    let block = be(4, 4);
    let ag_blocks = be(84, 4);
    let inode_size = be(104, 2);
    let (inopblog, agblklog) = (be(123, 1), be(124, 1));
    let root = be(56, 8);
    let ag = root >> (agblklog + inopblog);
    let ag_block = (root >> inopblog) & ((1 << agblklog) - 1);
    let slot = root & ((1 << inopblog) - 1);
    let inodes = (ag * ag_blocks + ag_block) * block + slot * inode_size;
    // The first block, by its magic number at `at`, of `len` bytes; or the
    // superblock, in a template without one, which no target uses.
    let find = |at: u64, len: usize, magic: u64| {
        image
            .sectors()
            .filter(|offset| offset % block == 0)
            .find(|&offset| offset > 0 && be(offset + at, len) == magic)
            .unwrap_or(0)
    };
    let directory = find(0, 4, 0x5844_3242);
    let leaves = find(8, 2, 0xD2FF);
    let bmap = find(0, 4, 0x424D_4150);
    let xattrs = find(8, 2, 0xFBEE);
    let log_start = be(48, 8);
    let log = ((log_start >> agblklog) * ag_blocks + (log_start & ((1 << agblklog) - 1))) * block;
    Template {
        image: Arc::new(image),
        superblock: 0,
        inodes,
        directory,
        leaves,
        bmap,
        xattrs,
        log,
    }
}

/// Lays `data` over `template` at `at`.
pub fn overlay(template: &Template, at: u64, data: &[u8]) -> Overlay {
    let mut overlay = Overlay::new(template.image.clone());
    overlay.patch(at, data);
    overlay
}
