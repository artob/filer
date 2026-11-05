// This is free and unencumbered software released into the public domain.

//! Valid volumes for the targets to lay their input over, so that the
//! fuzzer starts past the anchor's checks, and where their structures
//! are.

// Each target uses only part of it.
#![allow(dead_code)]

use crate::common::Overlay;
use std::sync::{Arc, OnceLock};

/// A template volume, and the positions of its structures.
pub struct Template {
    pub image: Arc<Vec<u8>>,
    /// The main volume descriptor sequence.
    pub descriptors: u64,
    /// The partition's first block.
    pub partition: u64,
    /// The last blocks, of a virtual allocation table.
    pub last: u64,
}

/// The volume of `make.sh`'s `udf-2.01`: the tree `tests/common`
/// expects, in blocks of 512 bytes.
pub fn plain() -> &'static Template {
    static TEMPLATE: OnceLock<Template> = OnceLock::new();
    TEMPLATE.get_or_init(|| build(include_bytes!("../../tests/fixtures/udf-2.01.sparse"), 512, 257))
}

/// The volume of `make.sh`'s `udf-2.50`: a BD-R's, of a virtual
/// partition, in blocks of 2 KiB.
pub fn write_once() -> &'static Template {
    static TEMPLATE: OnceLock<Template> = OnceLock::new();
    TEMPLATE.get_or_init(|| build(include_bytes!("../../tests/fixtures/udf-2.50.sparse"), 2048, 288))
}

/// Returns the template of sparse fixture `sparse`, of blocks of `block`
/// bytes, whose partition starts at block `start`.
fn build(sparse: &[u8], block: u64, start: u64) -> Template {
    let image = crate::common::image(sparse);
    let len = image.len() as u64;
    Template {
        image: Arc::new(image),
        descriptors: 96 * block,
        partition: start * block,
        last: len.saturating_sub(4 * block),
    }
}

/// Lays `data` over `template` at `at`.
pub fn overlay(template: &Template, at: u64, data: &[u8]) -> Overlay {
    let mut overlay = Overlay::new(template.image.clone());
    overlay.patch(at, data);
    overlay
}
