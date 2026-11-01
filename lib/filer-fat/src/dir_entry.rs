// This is free and unencumbered software released into the public domain.

//! Directory entries: decoding 32-byte slots, and assembling long names
//! with the short entries they belong to.
//!
//! A directory is a sequence of slots. A file or directory takes one short
//! slot, preceded by up to 20 long-name slots holding its name in UTF-16
//! fragments of 13 units, last fragment first. Each long slot carries a
//! checksum of the short name, so a long name left behind by software
//! unaware of long names is recognized as orphaned: per the specification,
//! it is then not part of any entry, which keeps its short name.

// The driver (FAT-04) uses the rest of this module.
#![allow(dead_code)]

use crate::{
    FatError, FatType, cp437,
    fat_time::Stamp,
    geometry::{Cluster, Geometry},
};
use alloc::{boxed::Box, string::String, vec::Vec};
use core::num::NonZeroU32;

/// The size of a directory slot, in bytes.
pub(crate) const SLOT: usize = 32;

/// A first byte ending the directory: this slot and all after are free.
const END: u8 = 0x00;

/// A first byte marking a free slot, as left by deleting its entry.
const FREE: u8 = 0xE5;

/// A first byte standing for a real `0xE5` in a short name.
const KANJI_E5: u8 = 0x05;

const ATTR_VOLUME_ID: u8 = 0x08;
const ATTR_DIRECTORY: u8 = 0x10;

/// The attributes of a long-name slot, under [`ATTR_LONG_NAME_MASK`].
const ATTR_LONG_NAME: u8 = 0x0F;
const ATTR_LONG_NAME_MASK: u8 = 0x3F;

/// The flag marking a long name's last fragment, which comes first.
const LAST_FRAGMENT: u8 = 0x40;

/// The most fragments a long name has: 20 of 13 units hold 255.
const MAX_FRAGMENTS: u8 = 20;

/// The UTF-16 units in one fragment.
const UNITS: usize = 13;

/// The UTF-16 units in [`MAX_FRAGMENTS`] fragments.
const MAX_UNITS: usize = 260;

/// The longest a long name may be, in UTF-16 units.
pub(crate) const MAX_LONG_NAME: usize = 255;

/// Where a long slot keeps its 13 units.
const UNIT_OFFSETS: [usize; UNITS] = [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30];

/// A short (8.3) name as stored: 11 bytes, an 8-byte base and a 3-byte
/// extension, each padded with spaces. A first byte of `0x05` stands for
/// `0xE5`; the stored bytes are kept, as the checksum covers them.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ShortName([u8; 11]);

impl ShortName {
    /// Returns the checksum that long-name slots carry for this name.
    pub(crate) fn checksum(&self) -> u8 {
        self.0
            .iter()
            .fold(0u8, |sum, &byte| sum.rotate_right(1).wrapping_add(byte))
    }

    /// Returns the stored bytes.
    pub(crate) const fn raw(&self) -> &[u8; 11] {
        &self.0
    }

    fn is_dot(&self) -> bool {
        &self.0 == b".          "
    }

    fn is_dot_dot(&self) -> bool {
        &self.0 == b"..         "
    }

    /// Returns the name as displayed: base and extension with their padding
    /// removed, joined by a dot if there is an extension, lowercased (ASCII
    /// only) per `case`, and decoded from code page 437.
    pub(crate) fn display(&self, case: Case) -> String {
        let [b0, b1, b2, b3, b4, b5, b6, b7, e0, e1, e2] = self.0;
        let b0 = if b0 == KANJI_E5 { FREE } else { b0 };
        let (base, ext) = ([b0, b1, b2, b3, b4, b5, b6, b7], [e0, e1, e2]);
        let (base, ext) = (trim(&base), trim(&ext));
        let decode = |byte, lower| cp437::decode(lower_if(byte, lower));
        let mut name = String::with_capacity(12);
        name.extend(base.iter().map(|&b| decode(b, case.lower_base())));
        if !ext.is_empty() {
            name.push('.');
            name.extend(ext.iter().map(|&b| decode(b, case.lower_ext())));
        }
        name
    }
}

fn trim(bytes: &[u8]) -> &[u8] {
    let len = bytes.iter().rposition(|&b| b != b' ').map_or(0, |at| {
        // An index is below the length, so this cannot overflow.
        at.saturating_add(1)
    });
    bytes.get(..len).unwrap_or_default()
}

const fn lower_if(byte: u8, lower: bool) -> u8 {
    if lower {
        byte.to_ascii_lowercase()
    } else {
        byte
    }
}

impl core::fmt::Debug for ShortName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ShortName({:?})", self.0.escape_ascii())
    }
}

/// The Windows NT case flags of a short entry without a long name: whether
/// its base and extension display in lowercase.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Case(u8);

impl Case {
    const LOWER_BASE: u8 = 0x08;
    const LOWER_EXT: u8 = 0x10;

    pub(crate) const fn lower_base(self) -> bool {
        self.0 & Self::LOWER_BASE != 0
    }

    pub(crate) const fn lower_ext(self) -> bool {
        self.0 & Self::LOWER_EXT != 0
    }
}

/// The attribute byte of a short entry, as stored. Whether the entry is a
/// directory or a volume label is decoded into [`Item`] and [`Body`]; the
/// rest is here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Attrs(u8);

impl Attrs {
    pub(crate) const fn read_only(self) -> bool {
        self.0 & 0x01 != 0
    }

    pub(crate) const fn hidden(self) -> bool {
        self.0 & 0x02 != 0
    }

    pub(crate) const fn system(self) -> bool {
        self.0 & 0x04 != 0
    }

    pub(crate) const fn archive(self) -> bool {
        self.0 & 0x20 != 0
    }

    pub(crate) const fn raw(self) -> u8 {
        self.0
    }
}

/// A long name: 1 to 255 UTF-16 units, none of them NUL. Unpaired
/// surrogates are possible, as FAT does not check for them.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct LongName(Box<[u16]>);

impl LongName {
    /// Returns the UTF-16 units.
    pub(crate) fn units(&self) -> &[u16] {
        &self.0
    }

    /// Returns the name as WTF-8: UTF-8, with any unpaired surrogate
    /// encoded as if it were a scalar value.
    pub(crate) fn to_wtf8(&self) -> Vec<u8> {
        let mut name = Vec::with_capacity(self.0.len());
        for unit in char::decode_utf16(self.0.iter().copied()) {
            match unit {
                Ok(c) => name.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes()),
                Err(error) => {
                    // Surrogates are 0xD800 to 0xDFFF: three bytes.
                    let [high, low] = error.unpaired_surrogate().to_be_bytes();
                    name.extend_from_slice(&[
                        0xE0 | (high >> 4),
                        0x80 | ((high & 0x0F) << 2) | (low >> 6),
                        0x80 | (low & 0x3F),
                    ]);
                },
            }
        }
        name
    }
}

impl core::fmt::Debug for LongName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let name: alloc::string::String = char::decode_utf16(self.0.iter().copied())
            .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect();
        write!(f, "LongName({name:?})")
    }
}

/// What a short entry holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Body {
    /// A directory, whose slots start at this cluster.
    Dir(Cluster),

    /// A file of no bytes, with no cluster.
    EmptyFile,

    /// A file of `size` bytes, whose data starts at this cluster.
    File { first: Cluster, size: NonZeroU32 },
}

/// A file or directory.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct DirEntry {
    long: Option<LongName>,
    short: ShortName,
    case: Case,
    attrs: Attrs,
    body: Body,
    created: Stamp,
    accessed: Stamp,
    modified: Stamp,
    /// The slots the entry takes: its first long slot (or its short slot,
    /// if it has no long name) and its short slot.
    slots: (u32, u32),
}

impl DirEntry {
    pub(crate) const fn long_name(&self) -> Option<&LongName> {
        self.long.as_ref()
    }

    pub(crate) const fn short_name(&self) -> &ShortName {
        &self.short
    }

    /// Returns the name to list: the long name as WTF-8 if there is one,
    /// else the short name, displayed per its case flags and so UTF-8.
    pub(crate) fn name(&self) -> Vec<u8> {
        match &self.long {
            Some(long) => long.to_wtf8(),
            None => self.short.display(self.case).into_bytes(),
        }
    }

    pub(crate) const fn attrs(&self) -> Attrs {
        self.attrs
    }

    pub(crate) const fn body(&self) -> Body {
        self.body
    }

    pub(crate) const fn created(&self) -> Stamp {
        self.created
    }

    pub(crate) const fn accessed(&self) -> Stamp {
        self.accessed
    }

    pub(crate) const fn modified(&self) -> Stamp {
        self.modified
    }

    /// Returns the index of the entry's first slot.
    pub(crate) const fn first_slot(&self) -> u32 {
        self.slots.0
    }

    /// Returns the index of the entry's short slot, which identifies it.
    pub(crate) const fn slot(&self) -> u32 {
        self.slots.1
    }
}

/// What a short slot is.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Item {
    /// The `.` entry of a subdirectory.
    Dot,

    /// The `..` entry of a subdirectory.
    DotDot,

    /// The volume label, which only the root directory has.
    Label(ShortName),

    /// A file or directory.
    Entry(DirEntry),
}

/// The result of feeding a slot to an [`Assembler`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Step {
    /// The directory ends; no slot from here on is in use.
    End,

    /// The slot is free, or a long-name fragment: read on.
    More,

    /// The slot completes an item.
    Item(Item),
}

/// A long-name slot, decoded.
struct Fragment {
    /// From 1 to [`MAX_FRAGMENTS`].
    order: u8,
    last: bool,
    checksum: u8,
    units: [u16; UNITS],
}

impl Fragment {
    /// Decodes a slot with long-name attributes, or returns `None` if it is
    /// not a valid fragment.
    fn decode(slot: &[u8; SLOT]) -> Option<Self> {
        #[rustfmt::skip]
        let [
            ord, _, _, _, _, _, _, _, _, _, _, _,
            kind, checksum, _, _, _, _, _, _, _, _, _, _, _, _,
            cluster_low, cluster_high, _, _, _, _,
        ] = *slot;
        let order = ord & !LAST_FRAGMENT;
        // The type and first cluster of a fragment are zero.
        if kind != 0 || cluster_low != 0 || cluster_high != 0 || order == 0 || order > MAX_FRAGMENTS
        {
            return None;
        }
        let mut units = [0; UNITS];
        for (unit, &at) in units.iter_mut().zip(&UNIT_OFFSETS) {
            let bytes = slot.get(at..)?.first_chunk::<2>()?;
            *unit = u16::from_le_bytes(*bytes);
        }
        Some(Self {
            order,
            last: ord & LAST_FRAGMENT != 0,
            checksum,
            units,
        })
    }
}

/// A long name being assembled.
struct Pending {
    /// The slot of its last fragment, which comes first.
    first_slot: u32,
    checksum: u8,
    /// The number of fragments.
    fragments: u8,
    /// The order of the fragment expected next; 0 once all have been read.
    next: u8,
    units: [u16; MAX_UNITS],
}

impl Pending {
    fn start(fragment: &Fragment, slot: u32) -> Option<Self> {
        let mut pending = Self {
            first_slot: slot,
            checksum: fragment.checksum,
            fragments: fragment.order,
            next: fragment.order,
            units: [0; MAX_UNITS],
        };
        pending.add(fragment)?;
        Some(pending)
    }

    /// Adds the next fragment, or returns `None` if it is not the next one.
    fn add(&mut self, fragment: &Fragment) -> Option<()> {
        if fragment.order != self.next || fragment.checksum != self.checksum {
            return None;
        }
        let at = usize::from(fragment.order.checked_sub(1)?).checked_mul(UNITS)?;
        self.units
            .get_mut(at..)?
            .first_chunk_mut::<UNITS>()?
            .copy_from_slice(&fragment.units);
        self.next = self.next.checked_sub(1)?;
        Some(())
    }

    /// Returns the long name, if it is complete and belongs to `short`.
    ///
    /// The name ends at the first NUL, which must lie in its last fragment,
    /// or fills that fragment; it is at most 255 units long.
    fn finish(&self, short: &ShortName) -> Option<LongName> {
        if self.next != 0 || self.checksum != short.checksum() {
            return None;
        }
        let capacity = usize::from(self.fragments).checked_mul(UNITS)?;
        let units = self.units.get(..capacity)?;
        let len = units.iter().position(|&unit| unit == 0).unwrap_or(capacity);
        let before_last = capacity.checked_sub(UNITS)?;
        if len <= before_last || len > MAX_LONG_NAME {
            return None;
        }
        Some(LongName(Box::from(units.get(..len)?)))
    }
}

/// Turns a directory's slots, fed in order, into its items.
///
/// Long names are matched with their short entries per the specification:
/// fragments must come in order, with the same checksum, ending right
/// before a short entry whose checksum matches. Anything else leaves the
/// fragments orphaned, and the entry with its short name.
pub(crate) struct Assembler<'a> {
    geometry: &'a Geometry,
    /// The directory's first cluster, or 0 for a FAT12/16 root directory;
    /// errors name it.
    dir: u32,
    /// The index of the next slot.
    slot: u32,
    pending: Option<Pending>,
    ended: bool,
}

impl<'a> Assembler<'a> {
    pub(crate) fn new(geometry: &'a Geometry, dir: u32) -> Self {
        Self {
            geometry,
            dir,
            slot: 0,
            pending: None,
            ended: false,
        }
    }

    /// Feeds the next slot.
    ///
    /// Fails if the slot is a short entry that no valid directory has: one
    /// that is both a directory and a label, or whose first cluster is
    /// invalid or inconsistent with its size.
    pub(crate) fn push(&mut self, slot: &[u8; SLOT]) -> Result<Step, FatError> {
        if self.ended {
            return Ok(Step::End);
        }
        let index = self.slot;
        self.slot = index
            .checked_add(1)
            .ok_or(FatError::DirectoryTooLarge { start: self.dir })?;
        let [first, _, _, _, _, _, _, _, _, _, _, attrs, ..] = *slot;
        // Anything but a fragment that continues a pending name leaves it
        // orphaned.
        let pending = self.pending.take();
        match first {
            END => {
                self.ended = true;
                return Ok(Step::End);
            },
            FREE => return Ok(Step::More),
            _ => {},
        }
        if attrs & ATTR_LONG_NAME_MASK == ATTR_LONG_NAME {
            if let Some(fragment) = Fragment::decode(slot) {
                self.pending = match pending {
                    Some(mut pending) if !fragment.last => pending.add(&fragment).map(|()| pending),
                    _ if fragment.last => Pending::start(&fragment, index),
                    _ => None,
                };
            }
            return Ok(Step::More);
        }
        self.short(slot, index, pending).map(Step::Item)
    }

    fn short(
        &self,
        slot: &[u8; SLOT],
        index: u32,
        pending: Option<Pending>,
    ) -> Result<Item, FatError> {
        #[rustfmt::skip]
        let [
            n0, n1, n2, n3, n4, n5, n6, n7, n8, n9, n10,
            attrs, case,
            created_centis, ct0, ct1, cd0, cd1,
            ad0, ad1,
            ch0, ch1,
            mt0, mt1, md0, md1,
            cl0, cl1,
            s0, s1, s2, s3,
        ] = *slot;
        let short = ShortName([n0, n1, n2, n3, n4, n5, n6, n7, n8, n9, n10]);
        if short.is_dot() {
            return Ok(Item::Dot);
        }
        if short.is_dot_dot() {
            return Ok(Item::DotDot);
        }
        let (dir, slot) = (self.dir, index);
        match attrs & (ATTR_DIRECTORY | ATTR_VOLUME_ID) {
            ATTR_VOLUME_ID => return Ok(Item::Label(short)),
            0 | ATTR_DIRECTORY => {},
            _ => return Err(FatError::BadAttributes { dir, slot, attrs }),
        }
        // The high half of the first cluster exists only on FAT32; on
        // FAT12/16 those bytes are reserved and not part of it.
        let high = match self.geometry.fat_type() {
            FatType::Fat32 => u32::from(u16::from_le_bytes([ch0, ch1])),
            FatType::Fat12 | FatType::Fat16 => 0,
        };
        let cluster = (high << 16) | u32::from(u16::from_le_bytes([cl0, cl1]));
        let first = match cluster {
            0 => None,
            n => Some(self.geometry.cluster(n).ok_or(FatError::BadEntryCluster {
                dir,
                slot,
                cluster,
            })?),
        };
        let size = NonZeroU32::new(u32::from_le_bytes([s0, s1, s2, s3]));
        let body = match (attrs & ATTR_DIRECTORY != 0, first, size) {
            // A directory's size is always zero, and never read.
            (true, Some(first), _) => Body::Dir(first),
            (false, Some(first), Some(size)) => Body::File { first, size },
            (false, None, None) => Body::EmptyFile,
            (false, Some(_), None) => {
                return Err(FatError::StrayCluster { dir, slot, cluster });
            },
            (true, None, _) | (false, None, Some(_)) => {
                return Err(FatError::MissingCluster { dir, slot });
            },
        };
        let long = pending.as_ref().and_then(|pending| pending.finish(&short));
        let first_slot = match (&long, &pending) {
            (Some(_), Some(pending)) => pending.first_slot,
            _ => index,
        };
        Ok(Item::Entry(DirEntry {
            long,
            short,
            case: Case(case),
            attrs: Attrs(attrs),
            body,
            created: Stamp::decode(
                u16::from_le_bytes([cd0, cd1]),
                u16::from_le_bytes([ct0, ct1]),
                created_centis,
            ),
            accessed: Stamp::decode(u16::from_le_bytes([ad0, ad1]), 0, 0),
            modified: Stamp::decode(
                u16::from_le_bytes([md0, md1]),
                u16::from_le_bytes([mt0, mt1]),
                0,
            ),
            slots: (first_slot, index),
        }))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::geometry::tests::Bpb;
    use alloc::{string::String, vec};

    /// Returns a short slot.
    pub(crate) fn short(name: &[u8; 11], attrs: u8, cluster: u32, size: u32) -> [u8; SLOT] {
        let mut slot = [0; SLOT];
        slot[..11].copy_from_slice(name);
        slot[11] = attrs;
        let cluster = cluster.to_le_bytes();
        slot[20..22].copy_from_slice(&cluster[2..]);
        slot[26..28].copy_from_slice(&cluster[..2]);
        slot[28..].copy_from_slice(&size.to_le_bytes());
        slot
    }

    /// Returns the long slots of `units`, as they precede `short` on disk.
    pub(crate) fn long_units(units: &[u16], fragments: usize, short: &[u8; 11]) -> Vec<[u8; SLOT]> {
        let mut padded = units.to_vec();
        if padded.len() < fragments * UNITS {
            padded.push(0);
        }
        padded.resize(fragments * UNITS, 0xFFFF);
        let checksum = ShortName(*short).checksum();
        (1..=fragments)
            .rev()
            .map(|order| {
                let mut slot = [0; SLOT];
                slot[0] = u8::try_from(order).unwrap()
                    | if order == fragments { LAST_FRAGMENT } else { 0 };
                slot[11] = ATTR_LONG_NAME;
                slot[13] = checksum;
                for (i, &at) in UNIT_OFFSETS.iter().enumerate() {
                    let unit = padded[(order - 1) * UNITS + i];
                    slot[at..at + 2].copy_from_slice(&unit.to_le_bytes());
                }
                slot
            })
            .collect()
    }

    /// Returns the long slots of `name`, as they precede `short` on disk.
    pub(crate) fn long(name: &str, short: &[u8; 11]) -> Vec<[u8; SLOT]> {
        let units: Vec<u16> = name.encode_utf16().collect();
        long_units(&units, units.len().div_ceil(UNITS), short)
    }

    fn floppy() -> Geometry {
        Geometry::parse(&Bpb::floppy().bytes()).unwrap()
    }

    /// Feeds `slots`, returning every step that is not `More`.
    fn assemble(geometry: &Geometry, slots: &[[u8; SLOT]]) -> Vec<Result<Step, FatError>> {
        let mut assembler = Assembler::new(geometry, 7);
        slots
            .iter()
            .map(|slot| assembler.push(slot))
            .filter(|step| step != &Ok(Step::More))
            .collect()
    }

    /// Feeds `slots`, returning the entries.
    fn entries(slots: &[[u8; SLOT]]) -> Vec<DirEntry> {
        assemble(&floppy(), slots)
            .into_iter()
            .map(|step| match step {
                Ok(Step::Item(Item::Entry(entry))) => entry,
                step => panic!("unexpected {step:?}"),
            })
            .collect()
    }

    fn name(entry: &DirEntry) -> String {
        String::from_utf8(entry.name()).unwrap()
    }

    #[test]
    fn computes_checksums_per_the_specification() {
        for name in [b"README  TXT", b"\x05BC     DE ", b"A~1     \xE9\xFF\x80"] {
            let mut sum: u8 = 0;
            for &byte in name {
                sum = (if sum & 1 == 1 { 0x80u8 } else { 0 })
                    .wrapping_add(sum >> 1)
                    .wrapping_add(byte);
            }
            assert_eq!(ShortName(*name).checksum(), sum);
        }
    }

    #[test]
    fn displays_short_names() {
        let display = |name: &[u8; 11], case: u8| ShortName(*name).display(Case(case));
        assert_eq!(display(b"README  TXT", 0), "README.TXT");
        assert_eq!(display(b"README  TXT", 0x08), "readme.TXT");
        assert_eq!(display(b"README  TXT", 0x18), "readme.txt");
        assert_eq!(display(b"MAKEFILE   ", 0x10), "MAKEFILE");
        assert_eq!(display(b"A B     C  ", 0), "A B.C");
        // 0x05 stands for 0xE5, which is σ in code page 437.
        assert_eq!(display(b"\x05BC     DE ", 0), "σBC.DE");
        // Lowercasing is ASCII only: 0x90 is É, and stays so.
        assert_eq!(display(b"\x90T      \x8A  ", 0x18), "Ét.è");
    }

    #[test]
    fn assembles_long_names() {
        let short_name = b"ALONGF~1TXT";
        let mut slots = long("A long file name.txt", short_name);
        assert_eq!(slots.len(), 2);
        slots.push(short(short_name, 0x20, 2, 100));
        let [entry] = &entries(&slots)[..] else {
            panic!()
        };
        assert_eq!(name(entry), "A long file name.txt");
        assert_eq!(entry.short_name(), &ShortName(*short_name));
        assert_eq!((entry.first_slot(), entry.slot()), (0, 2));
        assert!(entry.attrs().archive());

        // Names filling their last fragment have no terminator.
        for len in [13, 26, 255] {
            let name_of_len: String = "x".repeat(len);
            let mut slots = long(&name_of_len, short_name);
            slots.push(short(short_name, 0, 0, 0));
            assert_eq!(name(&entries(&slots)[0]), name_of_len);
        }
    }

    #[test]
    fn falls_back_to_short_names_for_orphaned_long_names() {
        let short_name = b"ALONGF~1TXT";
        let entry = short(short_name, 0, 0, 0);
        let fragments = long("A long file name.txt", short_name);
        let other = long("Another name", b"ANOTHE~1   ");
        let mut invalid = fragments[1];
        invalid[12] = 1;
        let mut deleted = fragments[1];
        deleted[0] = FREE;
        let free = [FREE; SLOT];
        let units: Vec<u16> = "short".encode_utf16().collect();
        let cases: [Vec<[u8; SLOT]>; 9] = [
            // No checksum match.
            vec![other[0], entry],
            // A missing fragment.
            vec![fragments[0], entry],
            vec![fragments[1], entry],
            // Out of order.
            vec![fragments[1], fragments[0], entry],
            // Interrupted.
            vec![fragments[0], free, fragments[1], entry],
            vec![fragments[0], deleted, entry],
            vec![fragments[0], invalid, entry],
            // Mixed with another name.
            vec![other[0], fragments[1], entry],
            // Ending before its last fragment.
            long_units(&units, 2, short_name),
        ];
        for (i, mut slots) in cases.into_iter().enumerate() {
            if i == 8 {
                slots.push(entry);
            }
            let found = entries(&slots);
            assert_eq!(found.len(), 1, "case {i}");
            assert_eq!(name(&found[0]), "ALONGF~1.TXT", "case {i}");
            assert_eq!(found[0].long_name(), None, "case {i}");
            assert_eq!(found[0].first_slot(), found[0].slot(), "case {i}");
        }

        // Too long: 256 units.
        let mut slots = long(&"x".repeat(256), short_name);
        slots.push(entry);
        assert_eq!(entries(&slots)[0].long_name(), None);

        // A later complete name wins over an orphan before it.
        let mut slots = vec![fragments[0]];
        slots.extend(&fragments);
        slots.push(entry);
        let found = entries(&slots);
        assert_eq!(name(&found[0]), "A long file name.txt");
        assert_eq!(found[0].first_slot(), 1);
    }

    #[test]
    fn encodes_unpaired_surrogates_as_wtf8() {
        // A low surrogate, a high one, and a valid pair.
        let short_name = b"X          ";
        let mut slots = long_units(&[0x61, 0xDC00, 0xD800, 0xD83D, 0xDE00], 1, short_name);
        slots.push(short(short_name, 0, 0, 0));
        assert_eq!(
            entries(&slots)[0].name(),
            [
                b'a', 0xED, 0xB0, 0x80, 0xED, 0xA0, 0x80, 0xF0, 0x9F, 0x98, 0x80
            ]
        );
    }

    #[test]
    fn classifies_slots() {
        let g = floppy();
        let steps = assemble(
            &g,
            &[
                short(b".          ", 0x10, 5, 0),
                short(b"..         ", 0x10, 0, 0),
                short(b"MY DISK    ", 0x08, 0, 0),
                [FREE; SLOT],
                short(b"SUB        ", 0x10, 9, 0),
                short(b"FILE    BIN", 0x01, 3, 1),
                [0; SLOT],
                short(b"AFTER      ", 0, 0, 0),
            ],
        );
        let [dot, dot_dot, label, sub, file, end, after] = &steps[..] else {
            panic!("{steps:?}")
        };
        assert_eq!(dot, &Ok(Step::Item(Item::Dot)));
        assert_eq!(dot_dot, &Ok(Step::Item(Item::DotDot)));
        assert_eq!(
            label,
            &Ok(Step::Item(Item::Label(ShortName(*b"MY DISK    "))))
        );
        let Ok(Step::Item(Item::Entry(sub))) = sub else {
            panic!()
        };
        assert_eq!(sub.body(), Body::Dir(g.cluster(9).unwrap()));
        assert_eq!(sub.slot(), 4);
        let Ok(Step::Item(Item::Entry(file))) = file else {
            panic!()
        };
        assert_eq!(
            file.body(),
            Body::File {
                first: g.cluster(3).unwrap(),
                size: NonZeroU32::MIN,
            }
        );
        assert!(file.attrs().read_only() && !file.attrs().hidden());
        assert_eq!((end, after), (&Ok(Step::End), &Ok(Step::End)));
    }

    #[test]
    fn decodes_times() {
        let mut slot = short(b"FILE       ", 0, 0, 0);
        slot[13] = 150;
        slot[14..16].copy_from_slice(&0x6000u16.to_le_bytes()); // 12:00:00
        slot[16..18].copy_from_slice(&0x5821u16.to_le_bytes()); // 2024-01-01
        slot[18..20].copy_from_slice(&0x5821u16.to_le_bytes());
        let entry = &entries(&[slot])[0];
        let created = entry
            .created()
            .to_timestamp(0, crate::fat_time::TEN_MILLIS)
            .unwrap();
        assert_eq!(
            (created.secs(), created.nanos()),
            (1_704_110_401, 500_000_000)
        );
        assert!(matches!(entry.accessed(), Stamp::Valid(_)));
        assert_eq!(entry.modified(), Stamp::Unset);
    }

    #[test]
    fn reads_the_high_cluster_half_only_on_fat32() {
        let fat32 = Geometry::parse(&Bpb::fat32().bytes()).unwrap();
        let slot = short(b"FILE       ", 0, 0x0001_0005, 1);
        let Ok(Step::Item(Item::Entry(entry))) = &assemble(&fat32, &[slot])[0] else {
            panic!()
        };
        assert_eq!(
            entry.body(),
            Body::File {
                first: fat32.cluster(0x0001_0005).unwrap(),
                size: NonZeroU32::MIN,
            }
        );
        let g = floppy();
        let Ok(Step::Item(Item::Entry(entry))) = &assemble(&g, &[slot])[0] else {
            panic!()
        };
        assert_eq!(
            entry.body(),
            Body::File {
                first: g.cluster(5).unwrap(),
                size: NonZeroU32::MIN,
            }
        );
    }

    #[test]
    fn rejects_inconsistent_entries() {
        let g = floppy();
        let (dir, slot) = (7, 0);
        for (entry, error) in [
            (
                short(b"SUB        ", 0x10, 0, 0),
                FatError::MissingCluster { dir, slot },
            ),
            (
                short(b"FILE       ", 0, 0, 1),
                FatError::MissingCluster { dir, slot },
            ),
            (
                short(b"FILE       ", 0, 3, 0),
                FatError::StrayCluster {
                    dir,
                    slot,
                    cluster: 3,
                },
            ),
            (
                short(b"FILE       ", 0, 1, 1),
                FatError::BadEntryCluster {
                    dir,
                    slot,
                    cluster: 1,
                },
            ),
            (
                short(b"FILE       ", 0, 2849, 1),
                FatError::BadEntryCluster {
                    dir,
                    slot,
                    cluster: 2849,
                },
            ),
            (
                short(b"ODD        ", 0x18, 0, 0),
                FatError::BadAttributes {
                    dir,
                    slot,
                    attrs: 0x18,
                },
            ),
        ] {
            assert_eq!(assemble(&g, &[entry]), [Err(error)]);
        }
    }
}
