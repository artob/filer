// This is free and unencumbered software released into the public domain.

//! Valid images for the targets to lay their input over, so that the
//! fuzzer starts past the superblock's checks, and where their tables
//! are.

// Each target uses only part of it.
#![allow(dead_code)]

use crate::common::Overlay;
use std::sync::{Arc, OnceLock};

/// A template image, and the positions of its tables.
pub struct Template {
    pub image: Arc<Vec<u8>>,
    /// The inode table.
    pub inodes: u64,
    /// The directory table.
    pub directories: u64,
    /// The extended attributes' names and values, if it has any.
    pub xattrs: u64,
}

/// The image of `make.sh`'s `spec`, with nothing compressed, so that the
/// input changes structures, not compressed bytes: files, directories,
/// symlinks, a FIFO, and attributes.
pub fn plain() -> &'static Template {
    static TEMPLATE: OnceLock<Template> = OnceLock::new();
    TEMPLATE.get_or_init(|| {
        build(include_bytes!(
            "../../tests/fixtures/spec-uncompressed.sqfs"
        ))
    })
}

/// The image of `make.sh`'s `attrs`, with nothing compressed: attributes
/// of each namespace, many, large, and shared.
pub fn attributed() -> &'static Template {
    static TEMPLATE: OnceLock<Template> = OnceLock::new();
    TEMPLATE.get_or_init(|| {
        build(include_bytes!(
            "../../tests/fixtures/attrs-uncompressed.sqfs"
        ))
    })
}

fn build(image: &[u8]) -> Template {
    let u64_at = |at: u64| {
        let at = at as usize;
        u64::from_le_bytes(image[at..at + 8].try_into().unwrap())
    };
    let xattr_ids = u64_at(56);
    Template {
        image: Arc::new(image.to_vec()),
        inodes: u64_at(64),
        directories: u64_at(72),
        xattrs: if xattr_ids == u64::MAX { 0 } else { u64_at(xattr_ids) },
    }
}

/// Lays `data` over `template` at `at`.
pub fn overlay(template: &Template, at: u64, data: &[u8]) -> Overlay {
    let mut overlay = Overlay::new(template.image.clone());
    overlay.patch(at, data);
    overlay
}
