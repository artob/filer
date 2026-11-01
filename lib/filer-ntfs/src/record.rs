// This is free and unencumbered software released into the public domain.

//! MFT records, and the attributes they hold.

use crate::{NtfsError, fixup};
use alloc::vec::Vec;

/// The type of an attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct AttrType(pub(crate) u32);

impl AttrType {
    pub(crate) const STANDARD_INFORMATION: Self = Self(0x10);
    pub(crate) const ATTRIBUTE_LIST: Self = Self(0x20);
    pub(crate) const FILE_NAME: Self = Self(0x30);
    pub(crate) const DATA: Self = Self(0x80);
    pub(crate) const INDEX_ROOT: Self = Self(0x90);
    pub(crate) const INDEX_ALLOCATION: Self = Self(0xA0);
    #[allow(dead_code, reason = "for index and MFT bitmaps, later")]
    pub(crate) const BITMAP: Self = Self(0xB0);
    pub(crate) const REPARSE_POINT: Self = Self(0xC0);
    /// Ends the attributes of a record.
    const END: u32 = 0xFFFF_FFFF;
}

/// A reference to an MFT record: its number (48 bits) and the sequence
/// number it must have, which changes each time the record is reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct FileRef {
    pub(crate) record: u64,
    pub(crate) sequence: u16,
}

impl FileRef {
    pub(crate) fn from_u64(value: u64) -> Self {
        let [a, b, c, d, e, f, s0, s1] = value.to_le_bytes();
        Self {
            record: u64::from_le_bytes([a, b, c, d, e, f, 0, 0]),
            sequence: u16::from_le_bytes([s0, s1]),
        }
    }
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(*bytes.get(at..)?.first_chunk()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(*bytes.get(at..)?.first_chunk()?))
}

fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(*bytes.get(at..)?.first_chunk()?))
}

/// A valid MFT record, with its update sequence removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Record {
    number: u64,
    bytes: Vec<u8>,
    sequence: u16,
    links: u16,
    flags: u16,
    /// The base record, if this is an extension record.
    base: Option<FileRef>,
    /// Where the attributes start and the used bytes end.
    attributes: usize,
    used: usize,
}

impl Record {
    const IN_USE: u16 = 0x0001;
    const DIRECTORY: u16 = 0x0002;

    /// Parses `bytes`, read as record `number`: checks its signature and
    /// update sequence, and that its header fits.
    pub(crate) fn parse(number: u64, mut bytes: Vec<u8>) -> Result<Self, NtfsError> {
        let bad = || NtfsError::BadMftRecord { record: number };
        if bytes.get(..4) != Some(b"FILE".as_slice()) {
            return Err(bad());
        }
        let usa_offset = u16_at(&bytes, 4).ok_or_else(bad)?;
        let usa_count = u16_at(&bytes, 6).ok_or_else(bad)?;
        if !fixup::apply(&mut bytes, usa_offset, usa_count) {
            return Err(NtfsError::TornRecord { record: number });
        }
        let attributes = usize::from(u16_at(&bytes, 0x14).ok_or_else(bad)?);
        let used = usize::try_from(u32_at(&bytes, 0x18).ok_or_else(bad)?).map_err(|_| bad())?;
        let usa_end =
            usize::from(usa_offset).saturating_add(usize::from(usa_count).saturating_mul(2));
        if attributes < usa_end
            || !attributes.is_multiple_of(8)
            || used > bytes.len()
            || attributes.checked_add(4).is_none_or(|end| end > used)
        {
            return Err(bad());
        }
        // NTFS 3.1 records their own number: a misplaced record is caught.
        if usize::from(usa_offset) >= 0x30 {
            let own = u32_at(&bytes, 0x2C).ok_or_else(bad)?;
            if u64::from(own) != number & 0xFFFF_FFFF {
                return Err(bad());
            }
        }
        // No base is the whole reference being zero: `$MFT`'s extension
        // records refer to record 0, with its sequence number.
        let base = u64_at(&bytes, 0x20).ok_or_else(bad)?;
        Ok(Self {
            number,
            sequence: u16_at(&bytes, 0x10).ok_or_else(bad)?,
            links: u16_at(&bytes, 0x12).ok_or_else(bad)?,
            flags: u16_at(&bytes, 0x16).ok_or_else(bad)?,
            base: (base != 0).then(|| FileRef::from_u64(base)),
            attributes,
            used,
            bytes,
        })
    }

    pub(crate) const fn number(&self) -> u64 {
        self.number
    }

    pub(crate) const fn sequence(&self) -> u16 {
        self.sequence
    }

    /// Returns the number of hard links (names) the file has.
    #[allow(dead_code, reason = "used by tests")]
    pub(crate) const fn links(&self) -> u16 {
        self.links
    }

    pub(crate) const fn in_use(&self) -> bool {
        self.flags & Self::IN_USE != 0
    }

    pub(crate) const fn is_directory(&self) -> bool {
        self.flags & Self::DIRECTORY != 0
    }

    /// Returns the base record of an extension record.
    pub(crate) const fn base(&self) -> Option<FileRef> {
        self.base
    }

    /// Returns the record's attributes, in order. Iteration stops at the
    /// first malformed one, which it yields as an error.
    pub(crate) fn attributes(&self) -> Attributes<'_> {
        Attributes {
            record: self,
            at: self.attributes,
            failed: false,
        }
    }
}

/// The attributes of a [`Record`].
pub(crate) struct Attributes<'a> {
    record: &'a Record,
    at: usize,
    failed: bool,
}

impl<'a> Iterator for Attributes<'a> {
    type Item = Result<Attribute<'a>, NtfsError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        let record = self.record;
        let used = record.bytes.get(..record.used).unwrap_or_default();
        let bad = NtfsError::BadAttribute {
            record: record.number,
            offset: u32::try_from(self.at).unwrap_or(u32::MAX),
        };
        let kind = u32_at(used, self.at);
        if kind == Some(AttrType::END) {
            return None;
        }
        let parsed = kind.and_then(|kind| {
            let len = usize::try_from(u32_at(used, self.at.checked_add(4)?)?).ok()?;
            let end = self.at.checked_add(len)?;
            let bytes = used.get(self.at..end)?;
            len.is_multiple_of(8).then_some(())?;
            Some((Attribute::parse(AttrType(kind), bytes)?, end))
        });
        match parsed {
            Some((attribute, end)) => {
                self.at = end;
                Some(Ok(attribute))
            },
            None => {
                self.failed = true;
                Some(Err(bad))
            },
        }
    }
}

/// An attribute in a record: its type, name, and body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Attribute<'a> {
    kind: AttrType,
    /// The name, as UTF-16LE bytes; empty for an unnamed attribute.
    name: &'a [u8],
    flags: u16,
    id: u16,
    body: Body<'a>,
}

/// Where an attribute's value is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Body<'a> {
    /// In the record.
    Resident(&'a [u8]),

    /// In clusters, mapped by data runs.
    NonResident(NonResident<'a>),
}

/// The header of a non-resident attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NonResident<'a> {
    /// The virtual clusters this extent of the attribute maps.
    pub(crate) first_vcn: u64,
    pub(crate) last_vcn: u64,
    /// The mapping pairs (data runs), undecoded.
    pub(crate) runs: &'a [u8],
    /// The compression unit, as a power of two of clusters; 0 if none.
    pub(crate) compression_unit: u16,
    /// Sizes in bytes, valid in the extent starting at VCN 0.
    pub(crate) allocated: u64,
    pub(crate) size: u64,
    pub(crate) initialized: u64,
}

impl<'a> Attribute<'a> {
    const COMPRESSED: u16 = 0x0001;
    const ENCRYPTED: u16 = 0x4000;
    #[allow(dead_code, reason = "used by tests")]
    const SPARSE: u16 = 0x8000;

    /// Parses an attribute's bytes, `kind` already read; `None` if any
    /// part does not fit.
    fn parse(kind: AttrType, bytes: &'a [u8]) -> Option<Self> {
        let non_resident = *bytes.get(8)?;
        let name_len = usize::from(*bytes.get(9)?).checked_mul(2)?;
        let name_at = usize::from(u16_at(bytes, 0x0A)?);
        let name = bytes.get(name_at..)?.get(..name_len)?;
        let flags = u16_at(bytes, 0x0C)?;
        let id = u16_at(bytes, 0x0E)?;
        let body = match non_resident {
            0 => {
                let len = usize::try_from(u32_at(bytes, 0x10)?).ok()?;
                let at = usize::from(u16_at(bytes, 0x14)?);
                Body::Resident(bytes.get(at..)?.get(..len)?)
            },
            1 => {
                let runs_at = usize::from(u16_at(bytes, 0x20)?);
                let header = NonResident {
                    first_vcn: u64_at(bytes, 0x10)?,
                    last_vcn: u64_at(bytes, 0x18)?,
                    runs: bytes.get(runs_at..)?,
                    compression_unit: u16_at(bytes, 0x22)?,
                    allocated: u64_at(bytes, 0x28)?,
                    size: u64_at(bytes, 0x30)?,
                    initialized: u64_at(bytes, 0x38)?,
                };
                // An empty attribute's last VCN is -1, stored as `u64::MAX`.
                if runs_at < 0x40 || header.last_vcn.wrapping_add(1) < header.first_vcn {
                    return None;
                }
                Body::NonResident(header)
            },
            _ => return None,
        };
        Some(Self {
            kind,
            name,
            flags,
            id,
            body,
        })
    }

    pub(crate) const fn kind(&self) -> AttrType {
        self.kind
    }

    /// Returns the name as UTF-16LE bytes, empty if unnamed.
    #[allow(dead_code, reason = "used by tests")]
    pub(crate) const fn name(&self) -> &'a [u8] {
        self.name
    }

    /// Returns `true` if the name is `name`, given as ASCII.
    pub(crate) fn is_named(&self, name: &str) -> bool {
        self.name.len() == name.len().saturating_mul(2)
            && self
                .name
                .as_chunks::<2>()
                .0
                .iter()
                .zip(name.bytes())
                .all(|(unit, byte)| *unit == [byte, 0])
    }

    #[allow(dead_code, reason = "used by tests")]
    pub(crate) const fn id(&self) -> u16 {
        self.id
    }

    pub(crate) const fn body(&self) -> Body<'a> {
        self.body
    }

    pub(crate) const fn is_compressed(&self) -> bool {
        self.flags & Self::COMPRESSED != 0
    }

    pub(crate) const fn is_encrypted(&self) -> bool {
        self.flags & Self::ENCRYPTED != 0
    }

    #[allow(dead_code, reason = "used by tests")]
    pub(crate) const fn is_sparse(&self) -> bool {
        self.flags & Self::SPARSE != 0
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::fixup::tests::protect;
    use alloc::{vec, vec::Vec};

    fn align8(n: usize) -> usize {
        n.div_ceil(8) * 8
    }

    fn utf16(name: &str) -> Vec<u8> {
        name.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    /// Builds MFT records, as a writer would.
    #[derive(Clone)]
    pub(crate) struct RecordBuilder {
        pub(crate) number: u64,
        pub(crate) size: usize,
        pub(crate) flags: u16,
        pub(crate) links: u16,
        pub(crate) sequence: u16,
        pub(crate) base: u64,
        pub(crate) attributes: Vec<Vec<u8>>,
    }

    impl RecordBuilder {
        pub(crate) fn new(number: u64) -> Self {
            Self {
                number,
                size: 1024,
                flags: Record::IN_USE,
                links: 1,
                sequence: 1,
                base: 0,
                attributes: Vec::new(),
            }
        }

        /// Adds a resident attribute.
        pub(crate) fn resident(mut self, kind: AttrType, name: &str, value: &[u8]) -> Self {
            let name = utf16(name);
            let value_at = align8(0x18 + name.len());
            let mut a = vec![0u8; align8(value_at + value.len())];
            a[0..4].copy_from_slice(&kind.0.to_le_bytes());
            let len = u32::try_from(a.len()).unwrap();
            a[4..8].copy_from_slice(&len.to_le_bytes());
            a[9] = u8::try_from(name.len() / 2).unwrap();
            a[0x0A..0x0C].copy_from_slice(&0x18u16.to_le_bytes());
            a[0x0E..0x10]
                .copy_from_slice(&u16::try_from(self.attributes.len()).unwrap().to_le_bytes());
            a[0x10..0x14].copy_from_slice(&u32::try_from(value.len()).unwrap().to_le_bytes());
            a[0x14..0x16].copy_from_slice(&u16::try_from(value_at).unwrap().to_le_bytes());
            a[0x18..0x18 + name.len()].copy_from_slice(&name);
            a[value_at..value_at + value.len()].copy_from_slice(value);
            self.attributes.push(a);
            self
        }

        /// Adds a non-resident attribute mapping VCNs `vcns` with `runs`,
        /// of `size` bytes (also allocated and initialized), with `flags`.
        pub(crate) fn non_resident(
            mut self,
            kind: AttrType,
            name: &str,
            vcns: (u64, u64),
            runs: &[u8],
            size: u64,
            flags: u16,
        ) -> Self {
            let name = utf16(name);
            let runs_at = align8(0x40 + name.len());
            let mut a = vec![0u8; align8(runs_at + runs.len())];
            a[0..4].copy_from_slice(&kind.0.to_le_bytes());
            let len = u32::try_from(a.len()).unwrap();
            a[4..8].copy_from_slice(&len.to_le_bytes());
            a[8] = 1;
            a[9] = u8::try_from(name.len() / 2).unwrap();
            a[0x0A..0x0C].copy_from_slice(&0x40u16.to_le_bytes());
            a[0x0C..0x0E].copy_from_slice(&flags.to_le_bytes());
            a[0x0E..0x10]
                .copy_from_slice(&u16::try_from(self.attributes.len()).unwrap().to_le_bytes());
            a[0x10..0x18].copy_from_slice(&vcns.0.to_le_bytes());
            a[0x18..0x20].copy_from_slice(&vcns.1.to_le_bytes());
            a[0x20..0x22].copy_from_slice(&u16::try_from(runs_at).unwrap().to_le_bytes());
            if flags & Attribute::COMPRESSED != 0 {
                a[0x22..0x24].copy_from_slice(&4u16.to_le_bytes());
            }
            for at in [0x28, 0x30, 0x38] {
                a[at..at + 8].copy_from_slice(&size.to_le_bytes());
            }
            a[0x40..0x40 + name.len()].copy_from_slice(&name);
            a[runs_at..runs_at + runs.len()].copy_from_slice(runs);
            self.attributes.push(a);
            self
        }

        /// Returns the record's bytes, protected by an update sequence.
        pub(crate) fn bytes(&self) -> Vec<u8> {
            let mut r = vec![0u8; self.size];
            let count = self.size / 512 + 1;
            let attributes_at = align8(0x30 + 2 * count);
            r[0..4].copy_from_slice(b"FILE");
            r[4..6].copy_from_slice(&0x30u16.to_le_bytes());
            r[6..8].copy_from_slice(&u16::try_from(count).unwrap().to_le_bytes());
            r[0x10..0x12].copy_from_slice(&self.sequence.to_le_bytes());
            r[0x12..0x14].copy_from_slice(&self.links.to_le_bytes());
            r[0x14..0x16].copy_from_slice(&u16::try_from(attributes_at).unwrap().to_le_bytes());
            r[0x16..0x18].copy_from_slice(&self.flags.to_le_bytes());
            r[0x20..0x28].copy_from_slice(&self.base.to_le_bytes());
            r[0x2C..0x30].copy_from_slice(
                &u32::try_from(self.number & 0xFFFF_FFFF)
                    .unwrap()
                    .to_le_bytes(),
            );
            let mut at = attributes_at;
            for a in &self.attributes {
                r[at..at + a.len()].copy_from_slice(a);
                at += a.len();
            }
            r[at..at + 4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
            let used = u32::try_from(align8(at + 4)).unwrap();
            r[0x18..0x1C].copy_from_slice(&used.to_le_bytes());
            r[0x1C..0x20].copy_from_slice(&u32::try_from(self.size).unwrap().to_le_bytes());
            protect(&mut r, 0x30, 3);
            r
        }

        pub(crate) fn parse(&self) -> Result<Record, NtfsError> {
            Record::parse(self.number, self.bytes())
        }
    }

    #[test]
    fn parses_records_and_their_attributes() {
        let record = RecordBuilder {
            flags: Record::IN_USE | Record::DIRECTORY,
            links: 2,
            sequence: 9,
            ..RecordBuilder::new(42)
        }
        .resident(AttrType::STANDARD_INFORMATION, "", &[1; 72])
        .resident(AttrType::DATA, "stream", b"named")
        .non_resident(
            AttrType::DATA,
            "",
            (0, 9),
            &[0x11, 10, 20, 0],
            40960,
            0x8000,
        )
        .parse()
        .unwrap();
        assert_eq!(
            (record.number(), record.sequence(), record.links()),
            (42, 9, 2)
        );
        assert!(record.in_use() && record.is_directory());
        assert_eq!(record.base(), None);
        let attributes: Vec<Attribute<'_>> = record.attributes().map(Result::unwrap).collect();
        assert_eq!(attributes.len(), 3);
        assert_eq!(attributes[0].kind(), AttrType::STANDARD_INFORMATION);
        assert_eq!(attributes[0].body(), Body::Resident(&[1; 72]));
        assert!(attributes[1].is_named("stream") && !attributes[1].is_named("strea"));
        assert_eq!(attributes[1].body(), Body::Resident(b"named"));
        assert_eq!(attributes[1].id(), 1);
        let Body::NonResident(data) = attributes[2].body() else {
            panic!()
        };
        assert!(attributes[2].name().is_empty() && attributes[2].is_sparse());
        assert!(!attributes[2].is_compressed() && !attributes[2].is_encrypted());
        assert_eq!((data.first_vcn, data.last_vcn), (0, 9));
        assert_eq!(&data.runs[..4], &[0x11, 10, 20, 0]);
        assert_eq!(
            (data.allocated, data.size, data.initialized),
            (40960, 40960, 40960)
        );
    }

    #[test]
    fn reads_extension_records_and_references() {
        let reference = 0x0005_0000_0000_002A;
        let record = RecordBuilder {
            base: reference,
            ..RecordBuilder::new(50)
        }
        .parse()
        .unwrap();
        let base = FileRef {
            record: 42,
            sequence: 5,
        };
        assert_eq!(record.base(), Some(base));
        assert_eq!(FileRef::from_u64(reference), base);
        assert_eq!(record.attributes().count(), 0);
    }

    #[test]
    fn rejects_malformed_records() {
        let good = RecordBuilder::new(7)
            .resident(AttrType::DATA, "", b"x")
            .bytes();
        let parse = |bytes: Vec<u8>| Record::parse(7, bytes);
        let bad = Err(NtfsError::BadMftRecord { record: 7 });
        let patched = |at: usize, value: &[u8]| {
            let mut bytes = good.clone();
            bytes[at..at + value.len()].copy_from_slice(value);
            bytes
        };
        assert_eq!(parse(patched(0, b"BAAD")), bad);
        assert_eq!(
            parse(patched(1022, &[9, 9])),
            Err(NtfsError::TornRecord { record: 7 })
        );
        // Attributes before the update sequence's end, or misaligned.
        assert_eq!(parse(patched(0x14, &0x30u16.to_le_bytes())), bad);
        assert_eq!(parse(patched(0x14, &0x3Cu16.to_le_bytes())), bad);
        // Used bytes beyond the record.
        assert_eq!(parse(patched(0x18, &2000u32.to_le_bytes())), bad);
        // Another record's own number.
        assert_eq!(parse(patched(0x2C, &8u32.to_le_bytes())), bad);
        assert_eq!(
            Record::parse(8, good.clone()),
            Err(NtfsError::BadMftRecord { record: 8 })
        );
    }

    #[test]
    fn stops_at_malformed_attributes() {
        let good = RecordBuilder::new(7)
            .resident(AttrType::DATA, "", b"x")
            .bytes();
        let attribute_at: usize = 0x38;
        let bad = NtfsError::BadAttribute {
            record: 7,
            offset: 0x38,
        };
        for (at, value) in [
            // A length of zero, which would loop, odd, or past the end.
            (attribute_at + 4, 0u32),
            (attribute_at + 4, 33),
            (attribute_at + 4, 4096),
        ] {
            let mut bytes = good.clone();
            bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
            let record = Record::parse(7, bytes).unwrap();
            let results: Vec<_> = record.attributes().collect();
            assert_eq!(results, [Err(bad)], "{value}");
        }
        // A resident value, or a name, beyond the attribute.
        for (offset, value) in [(0x10, 1000u16), (0x14, 1000), (0x0A, 1000)] {
            let mut bytes = good.clone();
            let at = attribute_at + offset;
            bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
            if offset == 0x0A {
                bytes[attribute_at + 9] = 1;
            }
            let record = Record::parse(7, bytes).unwrap();
            assert_eq!(record.attributes().next(), Some(Err(bad)), "{offset}");
        }
    }
}
