// This is free and unencumbered software released into the public domain.

//! The master file table, and files gathered from its records.

use crate::{
    NtfsError,
    geometry::Geometry,
    record::{AttrType, Attribute, Body, FileRef, Record},
    stream::{self, Stream},
};
use alloc::{collections::BTreeSet, vec::Vec};
use filer_api::{Error, OpKind, ReadAt};

/// The MFT: `$MFT`'s unnamed `$DATA`, read in records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Mft {
    stream: Stream,
    record_size: u32,
    records: u64,
}

/// An entry of an `$ATTRIBUTE_LIST`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ListEntry<'a> {
    kind: AttrType,
    name: &'a [u8],
    first_vcn: u64,
    record: FileRef,
}

/// Parses an `$ATTRIBUTE_LIST` value of file `record`.
fn list(value: &[u8], record: u64) -> Result<Vec<ListEntry<'_>>, NtfsError> {
    let bad = NtfsError::BadAttributeList { record };
    let mut entries = Vec::new();
    let mut at = 0usize;
    while at < value.len() {
        let entry = value.get(at..).ok_or(bad)?;
        let field = |offset: usize, len: usize| entry.get(offset..)?.get(..len);
        let u16_at = |offset| {
            field(offset, 2)?
                .first_chunk()
                .map(|b| u16::from_le_bytes(*b))
        };
        let u64_at = |offset| {
            field(offset, 8)?
                .first_chunk()
                .map(|b| u64::from_le_bytes(*b))
        };
        let kind = field(0, 4)
            .and_then(<[u8]>::first_chunk)
            .map(|b| u32::from_le_bytes(*b))
            .ok_or(bad)?;
        let len = usize::from(u16_at(4).ok_or(bad)?);
        let name_len = usize::from(*entry.get(6).ok_or(bad)?).saturating_mul(2);
        let name_at = usize::from(*entry.get(7).ok_or(bad)?);
        if len < 0x1A || len > entry.len() {
            return Err(bad);
        }
        let name = entry
            .get(..len)
            .and_then(|entry| entry.get(name_at..)?.get(..name_len))
            .ok_or(bad)?;
        entries.push(ListEntry {
            kind: AttrType(kind),
            name,
            first_vcn: u64_at(8).ok_or(bad)?,
            record: FileRef::from_u64(u64_at(0x10).ok_or(bad)?),
        });
        at = at.checked_add(len).ok_or(bad)?;
    }
    Ok(entries)
}

/// Returns the extents of `kind` named `name` in `records`, sorted by
/// first VCN.
fn extents<'a>(
    records: impl IntoIterator<Item = &'a Record>,
    kind: AttrType,
    name: &str,
) -> Result<Vec<Attribute<'a>>, NtfsError> {
    matching(records, |attribute| {
        attribute.kind() == kind && attribute.is_named(name)
    })
}

/// Returns the extents in `records` that `matches` selects, sorted by
/// first VCN.
fn matching<'a>(
    records: impl IntoIterator<Item = &'a Record>,
    matches: impl Fn(&Attribute<'a>) -> bool,
) -> Result<Vec<Attribute<'a>>, NtfsError> {
    let mut found = Vec::new();
    for record in records {
        for attribute in record.attributes() {
            let attribute = attribute?;
            if matches(&attribute) {
                found.push(attribute);
            }
        }
    }
    found.sort_by_key(|attribute| match attribute.body() {
        Body::Resident(_) => 0,
        Body::NonResident(header) => header.first_vcn,
    });
    Ok(found)
}

impl Mft {
    /// Loads the MFT of the volume in `source`: record 0, whose unnamed
    /// `$DATA` maps the rest, including any extents held in extension
    /// records, which are read through the extents found before them.
    pub(crate) async fn load<S: ReadAt + ?Sized>(
        source: &S,
        geometry: &Geometry,
    ) -> Result<Self, Error> {
        let op = OpKind::Open;
        let fail = |error: NtfsError| error.into_error(op);
        let record_size = geometry.record_size();
        let at = geometry
            .cluster_offset(geometry.mft())
            .ok_or_else(|| fail(NtfsError::BadMftCluster(geometry.mft())))?;
        let len = usize::try_from(record_size).map_err(|_| fail(NtfsError::BadRecordSize(0)))?;
        let base =
            Record::parse(0, stream::read_exact(source, at, len, op).await?).map_err(fail)?;
        if !base.in_use() || base.base().is_some() {
            return Err(fail(NtfsError::BadReference { record: 0 }));
        }
        let mut records = alloc::vec![base];
        let mut mft = Self::from_records(&records, geometry, record_size, false).map_err(fail)?;
        // `$MFT`'s extension records, by the VCN of the extent they hold.
        let entries: Vec<(u64, FileRef)> = match extents(&records, AttrType::ATTRIBUTE_LIST, "")
            .map_err(fail)?
            .as_slice()
        {
            [] => Vec::new(),
            [attribute] => {
                let stream = Stream::new(&[*attribute], geometry, 0).map_err(fail)?;
                let len = usize::try_from(stream.len())
                    .map_err(|_| fail(NtfsError::BadAttributeList { record: 0 }))?;
                let value = stream.read_exact(source, geometry, 0, len, op).await?;
                let mut entries: Vec<(u64, FileRef)> = list(&value, 0)
                    .map_err(fail)?
                    .into_iter()
                    .filter(|entry| entry.kind == AttrType::DATA && entry.name.is_empty())
                    .filter(|entry| entry.record.record != 0)
                    .map(|entry| (entry.first_vcn, entry.record))
                    .collect();
                entries.sort_unstable();
                entries.dedup_by_key(|(_, record)| *record);
                entries
            },
            _ => return Err(fail(NtfsError::BadAttributeList { record: 0 })),
        };
        for (_, reference) in entries {
            let extension = mft.read(source, geometry, reference.record, op).await?;
            if !extension.in_use()
                || extension.base()
                    != Some(FileRef {
                        record: 0,
                        sequence: records.first().map_or(0, Record::sequence),
                    })
            {
                return Err(fail(NtfsError::BadExtension {
                    record: 0,
                    extension: reference.record,
                }));
            }
            records.push(extension);
            mft = Self::from_records(&records, geometry, record_size, false).map_err(fail)?;
        }
        Self::from_records(&records, geometry, record_size, true).map_err(fail)
    }

    /// Builds the MFT from `$MFT`'s records so far, `complete` once all
    /// its extension records are read.
    fn from_records(
        records: &[Record],
        geometry: &Geometry,
        record_size: u32,
        complete: bool,
    ) -> Result<Self, NtfsError> {
        let extents = extents(records, AttrType::DATA, "")?;
        let stream = match complete {
            true => Stream::new(&extents, geometry, 0)?,
            false => Stream::partial(&extents, geometry, 0)?,
        };
        if !matches!(
            stream,
            Stream::NonResident {
                compression: None,
                ..
            }
        ) {
            return Err(NtfsError::BadRuns { record: 0 });
        }
        let records = stream
            .len()
            .checked_div(u64::from(record_size))
            .unwrap_or(0);
        Ok(Self {
            stream,
            record_size,
            records,
        })
    }

    /// Returns the number of records in the MFT.
    #[allow(dead_code, reason = "used by tests")]
    pub(crate) const fn records(&self) -> u64 {
        self.records
    }

    /// Reads and parses record `number`, whatever it holds.
    pub(crate) async fn read<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        geometry: &Geometry,
        number: u64,
        op: OpKind,
    ) -> Result<Record, Error> {
        let fail = |error: NtfsError| error.into_error(op);
        if number >= self.records {
            return Err(fail(NtfsError::BadReference { record: number }));
        }
        let size = u64::from(self.record_size);
        let off = number
            .checked_mul(size)
            .ok_or_else(|| fail(NtfsError::BadReference { record: number }))?;
        let len =
            usize::try_from(size).map_err(|_| fail(NtfsError::BadReference { record: number }))?;
        let bytes = self
            .stream
            .read_exact(source, geometry, off, len, op)
            .await?;
        Record::parse(number, bytes).map_err(fail)
    }

    /// Reads the file `reference` names: its base record, which must be in
    /// use and of the same sequence number, and the extension records its
    /// `$ATTRIBUTE_LIST` names, which must belong to it.
    pub(crate) async fn file<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        geometry: &Geometry,
        reference: FileRef,
        op: OpKind,
    ) -> Result<File, Error> {
        let number = reference.record;
        let fail = |error: NtfsError| error.into_error(op);
        let base = self.read(source, geometry, number, op).await?;
        if !base.in_use() || base.base().is_some() || base.sequence() != reference.sequence {
            return Err(fail(NtfsError::BadReference { record: number }));
        }
        let listed: BTreeSet<u64> = match extents([&base], AttrType::ATTRIBUTE_LIST, "")
            .map_err(fail)?
            .as_slice()
        {
            [] => BTreeSet::new(),
            [attribute] => {
                let stream = Stream::new(&[*attribute], geometry, number).map_err(fail)?;
                let len = usize::try_from(stream.len())
                    .map_err(|_| fail(NtfsError::BadAttributeList { record: number }))?;
                let value = stream.read_exact(source, geometry, 0, len, op).await?;
                list(&value, number)
                    .map_err(fail)?
                    .into_iter()
                    .map(|entry| entry.record.record)
                    .filter(|&record| record != number)
                    .collect()
            },
            _ => return Err(fail(NtfsError::BadAttributeList { record: number })),
        };
        let mut extensions = Vec::with_capacity(listed.len());
        for extension in listed {
            let record = self.read(source, geometry, extension, op).await?;
            if !record.in_use() || record.base() != Some(reference) {
                return Err(fail(NtfsError::BadExtension {
                    record: number,
                    extension,
                }));
            }
            extensions.push(record);
        }
        Ok(File { base, extensions })
    }
}

/// A file: its base record, and its extension records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct File {
    base: Record,
    extensions: Vec<Record>,
}

impl File {
    /// Returns the base record.
    pub(crate) const fn base(&self) -> &Record {
        &self.base
    }

    /// Returns the number of the base record.
    pub(crate) fn number(&self) -> u64 {
        self.base().number()
    }

    /// Returns the extents of the attribute of `kind` named `name`, sorted
    /// by first VCN; empty if there is none.
    pub(crate) fn extents(
        &self,
        kind: AttrType,
        name: &str,
    ) -> Result<Vec<Attribute<'_>>, NtfsError> {
        extents(self.records(), kind, name)
    }

    /// Returns the extents of the attribute of `kind` named `name`, given
    /// as UTF-16, sorted by first VCN; empty if there is none.
    pub(crate) fn extents_utf16(
        &self,
        kind: AttrType,
        name: &[u16],
    ) -> Result<Vec<Attribute<'_>>, NtfsError> {
        matching(self.records(), |attribute| {
            attribute.kind() == kind && attribute.is_named_utf16(name)
        })
    }

    /// Returns the names of the named attributes of `kind`, as UTF-16, each
    /// once (however many extents it has), sorted.
    pub(crate) fn names(&self, kind: AttrType) -> Result<BTreeSet<Vec<u16>>, NtfsError> {
        let mut names = BTreeSet::new();
        for record in self.records() {
            for attribute in record.attributes() {
                let attribute = attribute?;
                if attribute.kind() == kind && !attribute.name().is_empty() {
                    names.insert(attribute.name_utf16());
                }
            }
        }
        Ok(names)
    }

    /// Returns the base record, then the extension records.
    fn records(&self) -> impl Iterator<Item = &Record> {
        core::iter::once(&self.base).chain(&self.extensions)
    }

    /// Returns the value of the attribute of `kind` named `name`, if any.
    pub(crate) fn stream(
        &self,
        geometry: &Geometry,
        kind: AttrType,
        name: &str,
    ) -> Result<Option<Stream>, NtfsError> {
        let extents = self.extents(kind, name)?;
        if extents.is_empty() {
            return Ok(None);
        }
        Stream::new(&extents, geometry, self.number()).map(Some)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{geometry::tests::Boot, record::tests::RecordBuilder};
    use alloc::{vec, vec::Vec};
    use filer_api::ErrorKind;
    use filer_spec::block_on;

    /// Encodes data runs: each `(lcn, len)`, sparse if `lcn` is `None`.
    pub(crate) fn encode_runs(runs: &[(Option<u64>, u64)]) -> Vec<u8> {
        fn minimal(value: i128, signed: bool) -> Vec<u8> {
            let bytes = value.to_le_bytes();
            let mut len = 1;
            while len < 16 {
                let top = bytes[len - 1];
                let rest_is_sign = bytes[len..]
                    .iter()
                    .all(|&b| b == if value < 0 { 0xFF } else { 0 });
                let sign_ok = !signed || (top & 0x80 != 0) == (value < 0);
                if rest_is_sign && sign_ok {
                    break;
                }
                len += 1;
            }
            bytes[..len].to_vec()
        }
        let mut out = Vec::new();
        let mut previous = 0i128;
        for &(lcn, len) in runs {
            let len = minimal(i128::from(len), false);
            let offset = match lcn {
                Some(lcn) => {
                    let offset = minimal(i128::from(lcn) - previous, true);
                    previous = i128::from(lcn);
                    offset
                },
                None => Vec::new(),
            };
            out.push(u8::try_from(len.len() | offset.len() << 4).unwrap());
            out.extend(len);
            out.extend(offset);
        }
        out.push(0);
        out
    }

    /// A 10 MiB volume, 4 KiB clusters, 1 KiB records, in memory.
    pub(crate) struct Volume {
        pub(crate) bytes: Vec<u8>,
        pub(crate) geometry: Geometry,
        /// Where the MFT's clusters are: `(lcn, len)`.
        mft: Vec<(u64, u64)>,
    }

    impl Volume {
        /// A volume whose MFT has the clusters `mft`, the first at the
        /// boot sector's `$MFT` cluster; record 0 must be written.
        pub(crate) fn new(mft: &[(u64, u64)]) -> Self {
            let boot = Boot {
                mft: mft[0].0,
                ..Boot::new()
            }
            .bytes();
            let geometry = Geometry::parse(&boot).unwrap();
            let mut bytes = vec![0u8; usize::try_from(geometry.volume_len()).unwrap()];
            bytes[..512].copy_from_slice(&boot);
            Self {
                bytes,
                geometry,
                mft: mft.to_vec(),
            }
        }

        /// Returns record 0 for this MFT, `$MFT` with its `$DATA`.
        pub(crate) fn mft_record(&self) -> RecordBuilder {
            let clusters: u64 = self.mft.iter().map(|&(_, len)| len).sum();
            let runs: Vec<_> = self
                .mft
                .iter()
                .map(|&(lcn, len)| (Some(lcn), len))
                .collect();
            RecordBuilder::new(0).non_resident(
                AttrType::DATA,
                "",
                (0, clusters - 1),
                &encode_runs(&runs),
                clusters * 4096,
                0,
            )
        }

        /// Writes a record where the MFT maps it.
        pub(crate) fn write_record(&mut self, record: &RecordBuilder) -> &mut Self {
            let bytes = record.bytes();
            let mut off = record.number * 1024;
            for &(lcn, len) in &self.mft {
                if off < len * 4096 {
                    let at = usize::try_from(lcn * 4096 + off).unwrap();
                    self.bytes[at..at + bytes.len()].copy_from_slice(&bytes);
                    return self;
                }
                off -= len * 4096;
            }
            panic!("record {} is past the MFT", record.number);
        }

        /// Writes `data` at cluster `lcn`.
        pub(crate) fn write(&mut self, lcn: u64, data: &[u8]) -> &mut Self {
            let at = usize::try_from(lcn * 4096).unwrap();
            self.bytes[at..at + data.len()].copy_from_slice(data);
            self
        }

        pub(crate) fn mft(&self) -> Result<Mft, Error> {
            block_on(Mft::load(self.bytes.as_slice(), &self.geometry))
        }
    }

    fn ntfs_error(error: &Error) -> NtfsError {
        *core::error::Error::source(error)
            .and_then(|source| source.downcast_ref::<NtfsError>())
            .unwrap_or_else(|| panic!("no NTFS error in {error:?}"))
    }

    fn reference(record: u64) -> FileRef {
        FileRef {
            record,
            sequence: 1,
        }
    }

    #[test]
    fn loads_the_mft_and_reads_records() {
        let mut volume = Volume::new(&[(4, 4)]);
        volume
            .write_record(&volume.mft_record())
            .write_record(&RecordBuilder::new(5));
        let mft = volume.mft().unwrap();
        assert_eq!(mft.records(), 16);
        let read =
            |n| block_on(mft.read(volume.bytes.as_slice(), &volume.geometry, n, OpKind::Stat));
        assert_eq!(read(5).unwrap().number(), 5);
        // Never written, and past the MFT.
        assert_eq!(
            ntfs_error(&read(6).unwrap_err()),
            NtfsError::BadMftRecord { record: 6 }
        );
        assert_eq!(
            ntfs_error(&read(16).unwrap_err()),
            NtfsError::BadReference { record: 16 }
        );
    }

    #[test]
    fn reads_records_of_a_fragmented_mft() {
        let mut volume = Volume::new(&[(4, 2), (40, 2)]);
        volume
            .write_record(&volume.mft_record())
            .write_record(&RecordBuilder::new(9));
        let mft = volume.mft().unwrap();
        let record = block_on(mft.read(volume.bytes.as_slice(), &volume.geometry, 9, OpKind::Stat));
        assert_eq!(record.unwrap().number(), 9);
        // Record 9 is in the second extent, at cluster 40.
        assert_eq!(&volume.bytes[40 * 4096 + 1024..][..4], b"FILE");
    }

    /// Encodes `$ATTRIBUTE_LIST` entries: type, first VCN, record.
    fn attribute_list(entries: &[(AttrType, u64, u64)]) -> Vec<u8> {
        let mut out = Vec::new();
        for &(kind, vcn, record) in entries {
            let mut entry = [0u8; 0x20];
            entry[0..4].copy_from_slice(&kind.0.to_le_bytes());
            entry[4..6].copy_from_slice(&0x20u16.to_le_bytes());
            entry[7] = 0x1A;
            entry[8..16].copy_from_slice(&vcn.to_le_bytes());
            let reference = record | 1 << 48;
            entry[0x10..0x18].copy_from_slice(&reference.to_le_bytes());
            out.extend(entry);
        }
        out
    }

    #[test]
    fn follows_the_mft_into_its_extension_records() {
        // `$MFT`'s `$DATA` in two extents: VCNs 0-1 in record 0, at
        // cluster 4, and VCNs 2-3 in its extension record 3, at cluster
        // 40, which the first extent maps.
        let mut volume = Volume::new(&[(4, 2), (40, 2)]);
        let list = attribute_list(&[(AttrType::DATA, 0, 0), (AttrType::DATA, 2, 3)]);
        let base = RecordBuilder::new(0)
            .resident(AttrType::ATTRIBUTE_LIST, "", &list)
            .non_resident(
                AttrType::DATA,
                "",
                (0, 1),
                &encode_runs(&[(Some(4), 2)]),
                16384,
                0,
            );
        let extension = RecordBuilder {
            base: 1 << 48,
            ..RecordBuilder::new(3)
        }
        .non_resident(
            AttrType::DATA,
            "",
            (2, 3),
            &encode_runs(&[(Some(40), 2)]),
            16384,
            0,
        );
        volume
            .write_record(&base)
            .write_record(&extension)
            .write_record(&RecordBuilder::new(12));
        let mft = volume.mft().unwrap();
        assert_eq!(mft.records(), 16);
        let record =
            block_on(mft.read(volume.bytes.as_slice(), &volume.geometry, 12, OpKind::Stat));
        assert_eq!(record.unwrap().number(), 12);

        // An extension record of another file is refused.
        let stranger = RecordBuilder {
            base: 7 | 1 << 48,
            ..extension.clone()
        };
        volume.write_record(&stranger);
        assert_eq!(
            ntfs_error(&volume.mft().unwrap_err()),
            NtfsError::BadExtension {
                record: 0,
                extension: 3
            }
        );
    }

    #[test]
    fn gathers_files_from_their_records() {
        let mut volume = Volume::new(&[(4, 4)]);
        let list = attribute_list(&[(AttrType::DATA, 0, 6)]);
        let base = RecordBuilder::new(5).resident(AttrType::ATTRIBUTE_LIST, "", &list);
        let extension = RecordBuilder {
            base: 5 | 1 << 48,
            ..RecordBuilder::new(6)
        }
        .resident(AttrType::DATA, "", b"contents");
        volume
            .write_record(&volume.mft_record())
            .write_record(&base)
            .write_record(&extension);
        let mft = volume.mft().unwrap();
        let file = |reference| {
            block_on(mft.file(
                volume.bytes.as_slice(),
                &volume.geometry,
                reference,
                OpKind::Stat,
            ))
        };
        let found = file(reference(5)).unwrap();
        assert_eq!(found.number(), 5);
        let data = found.stream(&volume.geometry, AttrType::DATA, "").unwrap();
        assert_eq!(data, Some(Stream::Resident(b"contents".to_vec())));
        assert_eq!(
            found.stream(&volume.geometry, AttrType::DATA, "named"),
            Ok(None)
        );

        // A stale sequence number, an extension record, and a free record.
        let stale = FileRef {
            record: 5,
            sequence: 2,
        };
        assert_eq!(
            ntfs_error(&file(stale).unwrap_err()),
            NtfsError::BadReference { record: 5 }
        );
        assert_eq!(
            ntfs_error(&file(reference(6)).unwrap_err()),
            NtfsError::BadReference { record: 6 }
        );
        volume.write_record(&RecordBuilder {
            flags: 0,
            ..RecordBuilder::new(7)
        });
        let mft = volume.mft().unwrap();
        let free = block_on(mft.file(
            volume.bytes.as_slice(),
            &volume.geometry,
            reference(7),
            OpKind::Stat,
        ));
        assert_eq!(
            ntfs_error(&free.unwrap_err()),
            NtfsError::BadReference { record: 7 }
        );
    }

    #[test]
    fn reads_streams() {
        let mut volume = Volume::new(&[(4, 4)]);
        // 3 clusters at 100, a sparse cluster, then 1 at 50: 5 clusters,
        // of which 18000 bytes are data, the last 2000 not initialized.
        let runs = encode_runs(&[(Some(100), 3), (None, 1), (Some(50), 1)]);
        let mut file =
            RecordBuilder::new(5).non_resident(AttrType::DATA, "", (0, 4), &runs, 20480, 0x8000);
        // Sizes: allocated 20480, size 20000, initialized 18000.
        let data = file.attributes.last_mut().unwrap();
        data[0x30..0x38].copy_from_slice(&20000u64.to_le_bytes());
        data[0x38..0x40].copy_from_slice(&18000u64.to_le_bytes());
        volume
            .write_record(&volume.mft_record())
            .write_record(&file)
            .write(100, &[1; 3 * 4096])
            .write(50, &[5; 4096]);
        let mft = volume.mft().unwrap();
        let source = volume.bytes.as_slice();
        let found =
            block_on(mft.file(source, &volume.geometry, reference(5), OpKind::Read)).unwrap();
        let stream = found
            .stream(&volume.geometry, AttrType::DATA, "")
            .unwrap()
            .unwrap();
        assert_eq!(stream.len(), 20000);
        let read = |off, max| {
            block_on(stream.read(source, &volume.geometry, off, max, OpKind::Read)).unwrap()
        };
        // A read stops at the end of a run.
        assert_eq!(read(0, 100_000), vec![1; 3 * 4096]);
        assert_eq!(read(3 * 4096, 100), vec![0; 100]);
        assert_eq!(read(4 * 4096, 100_000), vec![5; 18000 - 4 * 4096]);
        assert_eq!(read(18000, 100_000), vec![0; 2000]);
        assert_eq!(read(20000, 10), Vec::<u8>::new());
        let all =
            block_on(stream.read_exact(source, &volume.geometry, 0, 20000, OpKind::Read)).unwrap();
        assert_eq!(&all[12288..16384], &[0; 4096]);
        assert!(
            block_on(stream.read_exact(source, &volume.geometry, 0, 20001, OpKind::Read)).is_err()
        );
    }

    #[test]
    fn rejects_inconsistent_streams() {
        let volume = Volume::new(&[(4, 4)]);
        let runs = encode_runs(&[(Some(100), 2)]);
        let build = |patch: &dyn Fn(&mut Vec<u8>)| {
            let mut builder =
                RecordBuilder::new(5).non_resident(AttrType::DATA, "", (0, 1), &runs, 8192, 0);
            patch(builder.attributes.last_mut().unwrap());
            let record = builder.parse().unwrap();
            let extents: Vec<_> = record.attributes().map(Result::unwrap).collect();
            Stream::new(&extents, &volume.geometry, 5)
        };
        assert!(build(&|_| {}).is_ok());
        let size = NtfsError::BadDataSize { record: 5 };
        // Initialized past the size, the size past the allocation, and an
        // allocation past the mapped clusters.
        assert_eq!(
            build(&|a| a[0x38..0x40].copy_from_slice(&9000u64.to_le_bytes())),
            Err(size)
        );
        assert_eq!(
            build(&|a| a[0x30..0x38].copy_from_slice(&9000u64.to_le_bytes())),
            Err(size)
        );
        assert_eq!(
            build(&|a| a[0x28..0x30].copy_from_slice(&9000u64.to_le_bytes())),
            Err(size)
        );
        // Runs shorter than the extent claims.
        assert_eq!(
            build(&|a| a[0x18..0x20].copy_from_slice(&2u64.to_le_bytes())),
            Err(NtfsError::BadRuns { record: 5 })
        );
        // An extent that does not start at VCN 0.
        assert_eq!(
            build(&|a| {
                a[0x10..0x18].copy_from_slice(&1u64.to_le_bytes());
                a[0x18..0x20].copy_from_slice(&2u64.to_le_bytes());
            }),
            Err(NtfsError::BadRuns { record: 5 })
        );
        // A compression unit over 64 KiB: 32 clusters of 4 KiB.
        let mut builder =
            RecordBuilder::new(5).non_resident(AttrType::DATA, "", (0, 1), &runs, 8192, 0x0001);
        builder.attributes.last_mut().unwrap()[0x22] = 5;
        let record = builder.parse().unwrap();
        let extents: Vec<_> = record.attributes().map(Result::unwrap).collect();
        assert_eq!(Stream::new(&extents, &volume.geometry, 5), Err(size));
    }

    #[test]
    fn reads_compressed_units() {
        let mut volume = Volume::new(&[(4, 4)]);
        // One unit of 16 clusters: one allocated, holding a compressed
        // chunk ("abc", then 9 bytes 3 back), and 15 sparse; then a unit
        // stored as is; then a sparse unit.
        let token = (2u16 << 12 | 6).to_le_bytes();
        // The header holds the chunk's size, 8, minus 3.
        let chunk = [
            0x05,
            0xB0,
            0b0000_1000,
            b'a',
            b'b',
            b'c',
            token[0],
            token[1],
        ];
        let runs = encode_runs(&[(Some(100), 1), (None, 15), (Some(120), 16), (None, 16)]);
        let size = 3 * 65536;
        let file =
            RecordBuilder::new(5).non_resident(AttrType::DATA, "", (0, 47), &runs, size, 0x0001);
        volume
            .write_record(&volume.mft_record())
            .write_record(&file)
            .write(100, &chunk)
            .write(120, &[9; 65536]);
        let mft = volume.mft().unwrap();
        let source = volume.bytes.as_slice();
        let found =
            block_on(mft.file(source, &volume.geometry, reference(5), OpKind::Read)).unwrap();
        let stream = found
            .stream(&volume.geometry, AttrType::DATA, "")
            .unwrap()
            .unwrap();
        let read = |off, max| {
            block_on(stream.read(source, &volume.geometry, off, max, OpKind::Read)).unwrap()
        };
        assert_eq!(&read(0, 12), b"abcabcabcabc");
        assert_eq!(read(10, 5), b"bc\0\0\0");
        // A read stops at the end of a compressed unit.
        assert_eq!(read(65530, 100), vec![0; 6]);
        assert_eq!(read(65536, 70000), vec![9; 65536]);
        assert_eq!(read(2 * 65536 + 5, 10), vec![0; 10]);
        let all = block_on(stream.read_exact(source, &volume.geometry, 0, 3 * 65536, OpKind::Read))
            .unwrap();
        assert_eq!(all.len(), 3 * 65536);

        // A compressed unit whose data is malformed.
        volume.write(100, &[0x03, 0xB0, 0b0000_0001, 0, 0, 0, 0, 0]);
        let mft = volume.mft().unwrap();
        let source = volume.bytes.as_slice();
        let found =
            block_on(mft.file(source, &volume.geometry, reference(5), OpKind::Read)).unwrap();
        let stream = found
            .stream(&volume.geometry, AttrType::DATA, "")
            .unwrap()
            .unwrap();
        let error =
            block_on(stream.read(source, &volume.geometry, 0, 10, OpKind::Read)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt);
    }

    #[test]
    fn encodes_runs_for_tests() {
        assert_eq!(
            encode_runs(&[(Some(0x5634), 0x18), (None, 0x10), (Some(0x5624), 8)]),
            [0x21, 0x18, 0x34, 0x56, 0x01, 0x10, 0x11, 0x08, 0xF0, 0x00]
        );
        assert_eq!(
            encode_runs(&[(Some(0x80), 1)]),
            [0x21, 0x01, 0x80, 0x00, 0x00]
        );
    }
}
