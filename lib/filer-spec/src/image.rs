// This is free and unencumbered software released into the public domain.

//! Volume images for the tests of drivers that read them: fixtures stored
//! sparse, and overlays that mutate an image without copying it.
//!
//! Fixtures are kept in the repository as `.sparse` files, which hold an
//! image's nonzero sectors only, as the images are mostly zeros: the magic
//! `FILER-SPARSE-1\n` or `FILER-SPARSE-2\n`, the image's size (`u64`,
//! little-endian), then for version 1 each nonzero 512-byte sector as its
//! index (`u32`) and bytes, and for version 2 each run of identical
//! nonzero sectors as its first index and count (two `u32`s) and the
//! sector's bytes. A [`Sparse`] reads one as it is; [`Sparse::to_vec`]
//! expands it.
//!
//! # Examples
//!
//! ```
//! use filer_spec::{Image, Overlay, Sparse};
//! use std::sync::Arc;
//!
//! let mut file = b"FILER-SPARSE-2\n".to_vec();
//! file.extend(2048u64.to_le_bytes());
//! file.extend([1, 0, 0, 0, 2, 0, 0, 0]); // Sectors 1 and 2 ...
//! file.extend([7; 512]); // ... hold sevens.
//! let image = Sparse::new(&file);
//! assert_eq!((image.length(), image.byte(511), image.byte(512)), (2048, 0, 7));
//!
//! let mut overlay = Overlay::new(Arc::new(image));
//! overlay.patch(1023, &[1, 2]);
//! overlay.truncate(1024);
//! assert_eq!((overlay.byte(1023), overlay.length()), (1, 1024));
//! ```

use alloc::{collections::BTreeMap, sync::Arc, vec, vec::Vec};
use core::future::Future;
use filer_api::{BufResult, IoBufMut, ReadAt};

/// The bytes of a volume image, read by offset.
pub trait Image: Send + Sync + 'static {
    /// Returns the image's length in bytes.
    fn length(&self) -> u64;

    /// Fills `out` from offset `at`; bytes past the end read as zeros.
    fn copy(&self, at: u64, out: &mut [u8]);

    /// Returns the byte at `at`, or zero past the end.
    fn byte(&self, at: u64) -> u8 {
        let mut byte = [0];
        self.copy(at, &mut byte);
        byte[0]
    }
}

impl Image for Vec<u8> {
    fn length(&self) -> u64 {
        self.len() as u64
    }

    fn copy(&self, at: u64, out: &mut [u8]) {
        let start = usize::try_from(at).unwrap_or(usize::MAX);
        let bytes = self.get(start..).unwrap_or_default();
        let n = out.len().min(bytes.len());
        out[..n].copy_from_slice(&bytes[..n]);
        out[n..].fill(0);
    }
}

/// The size of a sector of a sparse image.
const SECTOR: usize = 512;

/// A sparse fixture, read as it is: runs of identical nonzero sectors,
/// all others zero.
#[derive(Clone, Debug)]
pub struct Sparse {
    len: u64,
    /// Runs of sectors, ascending: the first, the count, and the bytes.
    runs: Vec<(u64, u64, [u8; SECTOR])>,
}

impl Sparse {
    /// Reads a `.sparse` file, of either version.
    ///
    /// # Panics
    ///
    /// If `file` is not one, as a fixture that does not read is a broken
    /// test.
    #[must_use]
    pub fn new(file: &[u8]) -> Self {
        let (runs_of, rest) = match (
            file.strip_prefix(b"FILER-SPARSE-1\n"),
            file.strip_prefix(b"FILER-SPARSE-2\n"),
        ) {
            (Some(rest), _) => (false, rest),
            (_, Some(rest)) => (true, rest),
            _ => panic!("not a sparse image"),
        };
        let (len, mut rest) = rest.split_at(8);
        let len = u64::from_le_bytes(len.try_into().expect("sparse size"));
        let u32_at = |bytes: &[u8], at: usize| {
            u64::from(u32::from_le_bytes(
                bytes[at..at + 4].try_into().expect("sparse run"),
            ))
        };
        let mut runs = Vec::new();
        while !rest.is_empty() {
            let (first, count, header) = match runs_of {
                true => (u32_at(rest, 0), u32_at(rest, 4), 8),
                false => (u32_at(rest, 0), 1, 4),
            };
            let sector = rest[header..header + SECTOR]
                .try_into()
                .expect("sparse sector");
            runs.push((first, count, sector));
            rest = &rest[header + SECTOR..];
        }
        Self { len, runs }
    }

    /// Returns sector `index`, if it is not zero.
    fn sector(&self, index: u64) -> Option<&[u8; SECTOR]> {
        let after = self.runs.partition_point(|&(first, _, _)| first <= index);
        let (first, count, sector) = self.runs.get(after.checked_sub(1)?)?;
        (index < first + count).then_some(sector)
    }

    /// Returns the offsets of the nonzero sectors, ascending.
    pub fn sectors(&self) -> impl Iterator<Item = u64> + '_ {
        self.runs
            .iter()
            .flat_map(|&(first, count, _)| (first..first + count).map(|i| i * SECTOR as u64))
    }

    /// Returns the whole image.
    #[must_use]
    pub fn to_vec(&self) -> Vec<u8> {
        let mut image = vec![0; usize::try_from(self.len).expect("image size")];
        self.copy(0, &mut image);
        image
    }
}

impl Image for Sparse {
    fn length(&self) -> u64 {
        self.len
    }

    fn copy(&self, at: u64, out: &mut [u8]) {
        let mut done = 0;
        while done < out.len() {
            let pos = at + done as u64;
            let within = (pos % SECTOR as u64) as usize;
            let n = (SECTOR - within).min(out.len() - done);
            match self.sector(pos / SECTOR as u64).filter(|_| pos < self.len) {
                Some(sector) => out[done..done + n].copy_from_slice(&sector[within..within + n]),
                None => out[done..done + n].fill(0),
            }
            done += n;
        }
    }
}

impl ReadAt for Sparse {
    fn read_at<B: IoBufMut>(
        &self,
        buf: B,
        off: u64,
    ) -> impl Future<Output = BufResult<usize, B>> + Send {
        core::future::ready(read_image(self, self.len, &BTreeMap::new(), buf, off))
    }

    fn size(&self) -> impl Future<Output = filer_api::Result<u64>> + Send {
        core::future::ready(Ok(self.len))
    }
}

/// A volume image as a base image with bytes patched over it, and
/// possibly cut short, so that mutating a large image costs nothing.
#[derive(Clone, Debug)]
pub struct Overlay<B> {
    base: Arc<B>,
    patches: BTreeMap<u64, u8>,
    len: u64,
}

impl<B: Image> Overlay<B> {
    /// Returns `base`, not yet patched.
    #[must_use]
    pub fn new(base: Arc<B>) -> Self {
        let len = base.length();
        Self {
            base,
            patches: BTreeMap::new(),
            len,
        }
    }

    /// Patches `bytes` over the image at `at`, within its length.
    pub fn patch(&mut self, at: u64, bytes: &[u8]) {
        for (at, &byte) in (at..self.len).zip(bytes) {
            self.patches.insert(at, byte);
        }
    }

    /// Cuts the image short at `len` bytes, if it is longer.
    pub fn truncate(&mut self, len: u64) {
        self.len = self.len.min(len);
    }
}

impl<B: Image> Image for Overlay<B> {
    fn length(&self) -> u64 {
        self.len
    }

    fn copy(&self, at: u64, out: &mut [u8]) {
        let n = (out.len() as u64).min(self.len.saturating_sub(at)) as usize;
        self.base.copy(at, &mut out[..n]);
        for (&patched, &byte) in self.patches.range(at..at + n as u64) {
            out[(patched - at) as usize] = byte;
        }
        out[n..].fill(0);
    }
}

impl<B: Image> ReadAt for Overlay<B> {
    fn read_at<C: IoBufMut>(
        &self,
        buf: C,
        off: u64,
    ) -> impl Future<Output = BufResult<usize, C>> + Send {
        core::future::ready(read_image(&*self.base, self.len, &self.patches, buf, off))
    }

    fn size(&self) -> impl Future<Output = filer_api::Result<u64>> + Send {
        core::future::ready(Ok(self.len))
    }
}

/// Reads `base`, cut at `len` and with `patches` over it, into `buf` at
/// `off`.
fn read_image<I: Image + ?Sized, C: IoBufMut>(
    base: &I,
    len: u64,
    patches: &BTreeMap<u64, u8>,
    mut buf: C,
    off: u64,
) -> BufResult<usize, C> {
    let result = buf.fill(&mut |spare| {
        let n = (spare.len() as u64).min(len.saturating_sub(off)) as usize;
        base.copy(off, &mut spare[..n]);
        for (&at, &byte) in patches.range(off..off + n as u64) {
            spare[(at - off) as usize] = byte;
        }
        Ok(n)
    });
    BufResult(result, buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_on;

    fn file(version: u8, runs: &[(u32, u32, u8)]) -> Vec<u8> {
        let mut file = alloc::format!("FILER-SPARSE-{version}\n").into_bytes();
        file.extend(8192u64.to_le_bytes());
        for &(first, count, fill) in runs {
            file.extend(first.to_le_bytes());
            if version == 2 {
                file.extend(count.to_le_bytes());
            }
            file.extend([fill; SECTOR]);
        }
        file
    }

    #[test]
    fn reads_both_versions_alike() {
        let one = Sparse::new(&file(1, &[(1, 1, 5), (2, 1, 5), (7, 1, 9)]));
        let two = Sparse::new(&file(2, &[(1, 2, 5), (7, 1, 9)]));
        assert_eq!(one.to_vec(), two.to_vec());
        let image = two.to_vec();
        assert_eq!(
            (image[511], image[512], image[1535], image[1536]),
            (0, 5, 5, 0)
        );
        assert!(image[7 * 512..8 * 512].iter().all(|&byte| byte == 9));
        assert!(image[8 * 512..].iter().all(|&byte| byte == 0));
        assert_eq!(two.sectors().collect::<Vec<_>>(), [512, 1024, 3584]);
    }

    #[test]
    fn reads_overlays_as_patched() {
        let base = Arc::new(Sparse::new(&file(2, &[(0, 1, 1)])));
        let mut overlay = Overlay::new(base);
        overlay.patch(510, &[7, 8, 9]);
        overlay.patch(8191, &[3, 4]);
        let BufResult(result, buf) = block_on(overlay.read_at(Vec::with_capacity(4), 509));
        assert_eq!((result.unwrap(), buf), (4, vec![1, 7, 8, 9]));
        overlay.truncate(600);
        assert_eq!(block_on(overlay.size()).unwrap(), 600);
        let BufResult(result, _) = block_on(overlay.read_at(Vec::with_capacity(100), 590));
        assert_eq!(result.unwrap(), 10);
        overlay.truncate(9000);
        assert_eq!(overlay.length(), 600);
    }
}
