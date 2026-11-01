// This is free and unencumbered software released into the public domain.

//! An attribute's value: resident bytes, or clusters mapped by data runs,
//! and reading it.

use crate::{
    NtfsError,
    geometry::Geometry,
    lznt1,
    record::{Attribute, Body},
    runs::{self, Run},
};
use alloc::{vec, vec::Vec};
use filer_api::{BufResult, Error, ErrorKind, OpKind, ReadAt};

/// The largest compression unit read, in bytes: 16 clusters of 4 KiB.
const MAX_UNIT: u64 = 64 * 1024;

/// An attribute's value, from all of its extents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Stream {
    /// The value, held in the record.
    Resident(Vec<u8>),

    /// The value in clusters: `size` bytes, of which those past
    /// `initialized` read as zeros.
    NonResident {
        runs: Vec<Run>,
        size: u64,
        initialized: u64,
        /// The compression unit in clusters, if compressed.
        compression: Option<u64>,
    },
}

impl Stream {
    /// Builds the value of an attribute from its extents, sorted by first
    /// VCN, all from file record `record` (for errors). A resident
    /// attribute has one extent; a non-resident one's must be contiguous
    /// from VCN 0, and its sizes consistent.
    pub(crate) fn new(
        extents: &[Attribute<'_>],
        geometry: &Geometry,
        record: u64,
    ) -> Result<Self, NtfsError> {
        Self::build(extents, geometry, record, true)
    }

    /// Builds the part of a value its extents so far map, as [`new`]
    /// does, but with the sizes cut to the clusters mapped: for reading
    /// `$MFT`'s extension records through the extents found before them.
    ///
    /// [`new`]: Self::new
    pub(crate) fn partial(
        extents: &[Attribute<'_>],
        geometry: &Geometry,
        record: u64,
    ) -> Result<Self, NtfsError> {
        Self::build(extents, geometry, record, false)
    }

    fn build(
        extents: &[Attribute<'_>],
        geometry: &Geometry,
        record: u64,
        complete: bool,
    ) -> Result<Self, NtfsError> {
        let bad_runs = NtfsError::BadRuns { record };
        let bad_size = NtfsError::BadDataSize { record };
        let (first, rest) = extents.split_first().ok_or(bad_runs)?;
        let header = match first.body() {
            Body::Resident(value) if rest.is_empty() => return Ok(Self::Resident(value.to_vec())),
            Body::Resident(_) => return Err(bad_runs),
            Body::NonResident(header) => header,
        };
        if header.first_vcn != 0 {
            return Err(bad_runs);
        }
        let mut all: Vec<Run> = Vec::new();
        let mut next = 0u64;
        for extent in extents {
            let Body::NonResident(extent) = extent.body() else {
                return Err(bad_runs);
            };
            if extent.first_vcn != next {
                return Err(bad_runs);
            }
            let decoded = runs::decode(
                extent.runs,
                extent.first_vcn,
                extent.last_vcn,
                geometry.clusters(),
            )
            .ok_or(bad_runs)?;
            next = extent.last_vcn.wrapping_add(1);
            all.extend(decoded);
        }
        let mapped = next
            .checked_mul(u64::from(geometry.cluster_size()))
            .ok_or(bad_size)?;
        if header.initialized > header.size
            || header.size > header.allocated
            || complete && header.allocated > mapped
        {
            return Err(bad_size);
        }
        let (size, initialized) = match complete {
            true => (header.size, header.initialized),
            false => (header.size.min(mapped), header.initialized.min(mapped)),
        };
        // Windows compresses in units of 16 clusters of at most 4 KiB; a
        // unit of over 64 KiB would make reads allocate without bound.
        let compression = (first.is_compressed() && header.compression_unit != 0)
            .then(|| {
                1u64.checked_shl(u32::from(header.compression_unit))
                    .filter(|&clusters| {
                        clusters
                            .checked_mul(u64::from(geometry.cluster_size()))
                            .is_some_and(|bytes| bytes <= MAX_UNIT)
                    })
                    .ok_or(bad_size)
            })
            .transpose()?;
        Ok(Self::NonResident {
            runs: all,
            size,
            initialized,
            compression,
        })
    }

    /// Returns the value's length in bytes.
    pub(crate) fn len(&self) -> u64 {
        match self {
            Self::Resident(value) => u64::try_from(value.len()).unwrap_or(u64::MAX),
            Self::NonResident { size, .. } => *size,
        }
    }

    /// Reads up to `max` bytes at `off`, never past the value's end: as
    /// many as are contiguous on the volume, or zeros where sparse or past
    /// the initialized size. Returns no bytes at or past the end.
    pub(crate) async fn read<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        geometry: &Geometry,
        off: u64,
        max: usize,
        op: OpKind,
    ) -> Result<Vec<u8>, Error> {
        let (runs, size, initialized) = match self {
            Self::Resident(value) => {
                let start = usize::try_from(off).unwrap_or(usize::MAX);
                let rest = value.get(start..).unwrap_or_default();
                return Ok(rest.get(..max.min(rest.len())).unwrap_or_default().to_vec());
            },
            Self::NonResident {
                runs,
                size,
                initialized,
                ..
            } => (runs, *size, *initialized),
        };
        let cap = |n: u64| usize::try_from(n).map_or(max, |n| n.min(max));
        let want = cap(size.saturating_sub(off));
        if want == 0 {
            return Ok(Vec::new());
        }
        if off >= initialized {
            return Ok(vec![0; want]);
        }
        let want = want.min(cap(initialized.saturating_sub(off)));
        let cluster_size = u64::from(geometry.cluster_size());
        let corrupt = || Error::new(ErrorKind::Corrupt, op);
        if let Self::NonResident {
            compression: Some(unit),
            ..
        } = self
            && let Some(bytes) = self
                .read_unit(source, geometry, *unit, off, want, op)
                .await?
        {
            return Ok(bytes);
        }
        let vcn = off.checked_div(cluster_size).ok_or_else(corrupt)?;
        let run = runs::find(runs, vcn).ok_or_else(corrupt)?;
        let within = off
            .checked_sub(run.vcn.checked_mul(cluster_size).ok_or_else(corrupt)?)
            .ok_or_else(corrupt)?;
        let contiguous = run
            .len
            .get()
            .checked_mul(cluster_size)
            .and_then(|len| len.checked_sub(within))
            .ok_or_else(corrupt)?;
        let n = want.min(cap(contiguous));
        let Some(lcn) = run.lcn else {
            return Ok(vec![0; n]);
        };
        let at = geometry
            .cluster_offset(lcn)
            .and_then(|at| at.checked_add(within))
            .ok_or_else(corrupt)?;
        read_exact(source, at, n, op).await
    }

    /// Reads up to `want` bytes at `off` from the compression unit of
    /// `unit` clusters holding it, if the unit is compressed or sparse:
    /// some of its clusters allocated and the rest sparse is LZNT1 data,
    /// none allocated is zeros. Returns `None` for a unit stored as is,
    /// which reads as any other data.
    async fn read_unit<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        geometry: &Geometry,
        unit: u64,
        off: u64,
        want: usize,
        op: OpKind,
    ) -> Result<Option<Vec<u8>>, Error> {
        let Self::NonResident { runs, .. } = self else {
            return Ok(None);
        };
        let corrupt = || Error::new(ErrorKind::Corrupt, op);
        let cluster_size = u64::from(geometry.cluster_size());
        let unit_bytes = unit.checked_mul(cluster_size).ok_or_else(corrupt)?;
        let index = off.checked_div(unit_bytes).ok_or_else(corrupt)?;
        let first_vcn = index.checked_mul(unit).ok_or_else(corrupt)?;
        let end_vcn = first_vcn.checked_add(unit).ok_or_else(corrupt)?;
        // The unit's allocated clusters, in order, and how many are sparse.
        let mut allocated = Vec::new();
        let mut sparse = 0u64;
        let mut vcn = first_vcn;
        while vcn < end_vcn {
            let run = runs::find(runs, vcn).ok_or_else(corrupt)?;
            let n = run
                .end()
                .min(end_vcn)
                .checked_sub(vcn)
                .ok_or_else(corrupt)?;
            match run.lcn {
                Some(lcn) => {
                    let skip = vcn.checked_sub(run.vcn).ok_or_else(corrupt)?;
                    allocated.push((lcn.checked_add(skip).ok_or_else(corrupt)?, n));
                },
                None => sparse = sparse.checked_add(n).ok_or_else(corrupt)?,
            }
            vcn = vcn.checked_add(n).ok_or_else(corrupt)?;
        }
        if sparse == 0 {
            return Ok(None);
        }
        let within = off
            .checked_sub(index.checked_mul(unit_bytes).ok_or_else(corrupt)?)
            .ok_or_else(corrupt)?;
        let n = usize::try_from(unit_bytes.checked_sub(within).ok_or_else(corrupt)?)
            .map_or(want, |left| left.min(want));
        let unit_len = usize::try_from(unit_bytes).map_err(|_| corrupt())?;
        let mut output = vec![0; unit_len];
        if !allocated.is_empty() {
            let mut input = Vec::new();
            for (lcn, clusters) in allocated {
                let at = geometry.cluster_offset(lcn).ok_or_else(corrupt)?;
                let len = clusters
                    .checked_mul(cluster_size)
                    .and_then(|len| usize::try_from(len).ok())
                    .ok_or_else(corrupt)?;
                input.extend(read_exact(source, at, len, op).await?);
            }
            lznt1::decompress(&input, &mut output).ok_or_else(corrupt)?;
        }
        let start = usize::try_from(within).map_err(|_| corrupt())?;
        let end = start.checked_add(n).ok_or_else(corrupt)?;
        Ok(Some(output.get(start..end).ok_or_else(corrupt)?.to_vec()))
    }

    /// Reads exactly `len` bytes at `off`, or fails if the value ends first.
    pub(crate) async fn read_exact<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        geometry: &Geometry,
        off: u64,
        len: usize,
        op: OpKind,
    ) -> Result<Vec<u8>, Error> {
        let mut bytes = Vec::with_capacity(len);
        while bytes.len() < len {
            let at = off
                .checked_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX))
                .ok_or_else(|| Error::new(ErrorKind::Corrupt, op))?;
            let chunk = self
                .read(source, geometry, at, len.saturating_sub(bytes.len()), op)
                .await?;
            if chunk.is_empty() {
                return Err(Error::new(ErrorKind::Corrupt, op));
            }
            bytes.extend(chunk);
        }
        Ok(bytes)
    }
}

/// Reads exactly `len` bytes of the volume at `at`. The volume lies within
/// the source, as opening checked, so running short means the source
/// shrank, and that error stands as it is.
pub(crate) async fn read_exact<S: ReadAt + ?Sized>(
    source: &S,
    at: u64,
    len: usize,
    op: OpKind,
) -> Result<Vec<u8>, Error> {
    let BufResult(result, bytes) = source.read_exact_at(Vec::with_capacity(len), at).await;
    result.map_err(|error| match error.op() {
        OpKind::Read if op != OpKind::Read => Error::new(error.kind(), op),
        _ => error,
    })?;
    Ok(bytes)
}
