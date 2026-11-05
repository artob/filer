// This is free and unencumbered software released into the public domain.

//! Valid images for the targets to lay their input over, so that the
//! fuzzer starts past the descriptors' checks, and where their structures
//! are.

// Each target uses only part of it.
#![allow(dead_code)]

use crate::common::Overlay;
use std::sync::{Arc, OnceLock};

/// Where the volume descriptor set starts.
pub const DESCRIPTORS: u64 = 16 * 2048;

/// What begins a file zisofs compresses.
const ZISOFS: [u8; 8] = [0x37, 0xE4, 0x53, 0x96, 0xC9, 0xDB, 0xD6, 0x07];

/// A template image, and the positions of its structures.
pub struct Template {
    pub image: Arc<Vec<u8>>,
    /// The root directory of the tree read.
    pub root: u64,
    /// The first file zisofs compresses, if any.
    pub file: u64,
}

/// The image of `make.sh`'s `rock-ridge`: the tree `tests/common`
/// expects, with Rock Ridge's entries.
pub fn rock_ridge() -> &'static Template {
    static TEMPLATE: OnceLock<Template> = OnceLock::new();
    TEMPLATE.get_or_init(|| build(include_bytes!("../../tests/fixtures/rock-ridge.iso"), 1))
}

/// The image of `make.sh`'s `joliet`, whose Joliet tree is read.
pub fn joliet() -> &'static Template {
    static TEMPLATE: OnceLock<Template> = OnceLock::new();
    TEMPLATE.get_or_init(|| build(include_bytes!("../../tests/fixtures/joliet.iso"), 2))
}

/// The image of `make.sh`'s `zisofs`, every file it shrinks compressed.
pub fn zisofs() -> &'static Template {
    static TEMPLATE: OnceLock<Template> = OnceLock::new();
    TEMPLATE.get_or_init(|| build(include_bytes!("../../tests/fixtures/zisofs.iso"), 1))
}

/// Returns the template of `image`, the root that the first descriptor
/// of type `kind` names.
fn build(image: &[u8], kind: u8) -> Template {
    let u32_at = |at: usize| u32::from_le_bytes(image[at..at + 4].try_into().unwrap());
    let descriptor = (16..)
        .map(|sector| sector * 2048)
        .find(|&at| image[at] == kind)
        .unwrap();
    let root = u64::from(u32_at(descriptor + 156 + 2)) * 2048;
    let file = image
        .windows(ZISOFS.len())
        .position(|window| window == ZISOFS)
        .map_or(0, |at| at as u64);
    Template {
        image: Arc::new(image.to_vec()),
        root,
        file,
    }
}

/// Lays `data` over `template` at `at`.
pub fn overlay(template: &Template, at: u64, data: &[u8]) -> Overlay {
    let mut overlay = Overlay::new(template.image.clone());
    overlay.patch(at, data);
    overlay
}
