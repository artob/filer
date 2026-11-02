// This is free and unencumbered software released into the public domain.

//! A valid volume for the targets to lay their input over, so that the
//! fuzzer starts past the superblock's checks, and where its structures
//! are.

// Each target uses only part of it.
#![allow(dead_code)]

use crate::common::{exercise::Overlay, unsparse};
use std::sync::{Arc, OnceLock};

/// A template volume, and the offsets of its structures.
pub struct Template {
    pub image: Arc<Vec<u8>>,
    /// The superblock, which the group descriptors follow.
    pub superblock: u64,
    /// The inode table of the first group.
    pub inodes: u64,
    /// The root directory's first block.
    pub root: u64,
    /// The first extended attribute block.
    pub xattrs: u64,
}

/// The volume of `make.sh`'s `attrs`, with 4 KiB blocks and 256-byte
/// inodes: files, directories, and attributes in inodes and blocks.
pub fn plain() -> &'static Template {
    static TEMPLATE: OnceLock<Template> = OnceLock::new();
    TEMPLATE.get_or_init(|| build(include_bytes!("../../tests/fixtures/ext2-attrs.sparse")))
}

fn build(sparse: &[u8]) -> Template {
    let image = unsparse(sparse);
    let u32_at = |at: u64| {
        let at = at as usize;
        u64::from(u32::from_le_bytes(image[at..at + 4].try_into().unwrap()))
    };
    let block = 1024 << u32_at(1024 + 0x18);
    let descriptors = (u32_at(1024 + 0x14) + 1) * block;
    let inodes = u32_at(descriptors + 8) * block;
    let inode_size = u64::from(u16::from_le_bytes([image[1024 + 0x58], image[1024 + 0x59]]));
    // Inode 2, the root: its first block.
    let root = u32_at(inodes + inode_size + 0x28) * block;
    let xattrs = (1..image.len() as u64 / block)
        .map(|i| i * block)
        .find(|&at| u32_at(at) == 0xEA02_0000)
        .unwrap_or(0);
    Template {
        image: Arc::new(image),
        superblock: 1024,
        inodes,
        root,
        xattrs,
    }
}

/// Lays `data` over `template` at `at`.
pub fn overlay(template: &Template, at: u64, data: &[u8]) -> Overlay {
    let mut overlay = Overlay::new(template.image.clone());
    overlay.patch(at, data);
    overlay
}
