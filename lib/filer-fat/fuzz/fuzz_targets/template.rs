// This is free and unencumbered software released into the public domain.

//! Valid volumes for the targets to lay their input over, so that the
//! fuzzer starts past the boot sector's checks, and where their
//! structures are.

// Each target uses only part of it.
#![allow(dead_code)]

use crate::common::{Image, KINDS, dir, exercise::Overlay, file};
use std::sync::{Arc, OnceLock};

/// A template volume, and the offsets of its structures.
pub struct Template {
    pub image: Arc<Vec<u8>>,
    /// The first FAT.
    pub fat: u64,
    /// The root directory: its fixed region on FAT12/16, else its first
    /// cluster.
    pub root: u64,
}

/// Returns the template that `selector` picks: FAT12, FAT16, or FAT32.
pub fn template(selector: u8) -> &'static Template {
    static TEMPLATES: OnceLock<Vec<Template>> = OnceLock::new();
    let templates = TEMPLATES.get_or_init(|| KINDS.into_iter().map(build).collect());
    &templates[usize::from(selector) % templates.len()]
}

fn build(kind: crate::common::Kind) -> Template {
    let tree = vec![
        file("A long file name.txt", vec![7; 1500]),
        file("SHORT.TXT", "short"),
        dir(
            "Sub",
            vec![file("nested.bin", vec![9; 3000]), dir("Deeper", vec![])],
        ),
    ];
    let image = Image::fragmented(kind, tree);
    let bytes = &image.bytes;
    let u16_at = |at: usize| u64::from(u16::from_le_bytes([bytes[at], bytes[at + 1]]));
    let sector = u16_at(11);
    let fat = u16_at(14) * sector;
    let fat_len = match u16_at(22) {
        0 => u64::from(u32::from_le_bytes(bytes[36..40].try_into().unwrap())),
        sectors => sectors,
    } * sector;
    // The FATs are followed by the fixed root of FAT12/16, or by the data
    // region, whose first cluster is the FAT32 root: either way, the root.
    let root = fat + u64::from(bytes[16]) * fat_len;
    Template {
        image: Arc::new(image.bytes),
        fat,
        root,
    }
}

/// Lays `data` over `template` at `at`.
pub fn overlay(template: &Template, at: u64, data: &[u8]) -> Overlay {
    let mut overlay = Overlay::new(template.image.clone());
    overlay.patch(at, data);
    overlay
}
