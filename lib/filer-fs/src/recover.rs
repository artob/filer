// This is free and unencumbered software released into the public domain.

//! Recovery in memory: what replaying a dirty journal or log would write
//! to a volume, kept in an [`Overlay`] instead, and laid over what the
//! volume holds by [`Recovered`], so that it reads as recovery would
//! leave it, though nothing is written to it.
//!
//! A volume shut down before its journal was written back holds the
//! journal's last transactions twice: in the journal, committed, and in
//! place, perhaps half written. Recovery copies them from the journal to
//! their places, as Linux does when it mounts the volume; a driver here,
//! reading only, replays them into an overlay of its source instead.
//!
//! # Examples
//!
//! ```
//! # filer_spec::block_on(async {
//! use filer_api::{BufResult, ReadAt};
//! use filer_fs::recover::{Overlay, Recovered};
//!
//! let mut overlay = Overlay::new();
//! assert!(overlay.put(128, &[7; 128]));
//! let volume = Recovered::new(vec![0u8; 512], overlay);
//! let BufResult(read, buf) = volume.read_at(Vec::with_capacity(4), 126).await;
//! assert_eq!((read.unwrap(), buf), (4, vec![0, 0, 7, 7]));
//! # });
//! ```

use alloc::{collections::BTreeMap, vec::Vec};
use core::future::Future;
use filer_api::{BufResult, IoBufMut, ReadAt};

/// The size of what an overlay keeps: 128-byte chunks, the smallest unit
/// the formats here journal or log.
pub const CHUNK: usize = 128;
const CHUNK_SIZE: u64 = 128;

/// Bytes replayed over a volume's: whole [`CHUNK`]s, by their offsets,
/// the latest put of each kept.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Overlay {
    chunks: BTreeMap<u64, [u8; CHUNK]>,
}

impl Overlay {
    /// Returns an empty overlay, which changes nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            chunks: BTreeMap::new(),
        }
    }

    /// Lays the chunks over `out`, which was read from offset `at`.
    pub fn lay(&self, at: u64, out: &mut [u8]) {
        let len = u64::try_from(out.len()).unwrap_or(u64::MAX);
        let first = at
            .checked_div(CHUNK_SIZE)
            .unwrap_or(0)
            .saturating_mul(CHUNK_SIZE);
        for (&chunk, bytes) in self.chunks.range(first..at.saturating_add(len)) {
            for (i, &byte) in bytes.iter().enumerate() {
                let pos = chunk.saturating_add(u64::try_from(i).unwrap_or(0));
                if let Some(slot) = pos
                    .checked_sub(at)
                    .and_then(|i| usize::try_from(i).ok())
                    .and_then(|i| out.get_mut(i))
                {
                    *slot = byte;
                }
            }
        }
    }

    /// Puts `bytes` at offset `at`, over what was put there before.
    /// Returns `false`, putting nothing, unless `at` is a multiple of
    /// [`CHUNK`] and `bytes` whole chunks, ending within `u64`.
    #[must_use]
    pub fn put(&mut self, at: u64, bytes: &[u8]) -> bool {
        let (chunks, rest) = bytes.as_chunks::<CHUNK>();
        let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if !rest.is_empty() || !at.is_multiple_of(CHUNK_SIZE) || at.checked_add(len).is_none() {
            return false;
        }
        let mut offset = at;
        for chunk in chunks {
            self.chunks.insert(offset, *chunk);
            offset = offset.saturating_add(CHUNK_SIZE);
        }
        true
    }

    /// Returns `true` if nothing is put.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }
}

/// A source, with an [`Overlay`] laid over what it reads.
#[derive(Clone, Debug)]
pub struct Recovered<S> {
    source: S,
    overlay: Overlay,
}

impl<S> Recovered<S> {
    /// Returns `source`, read with `overlay` laid over it.
    pub const fn new(source: S, overlay: Overlay) -> Self {
        Self { source, overlay }
    }

    /// Returns the source, without the overlay.
    pub const fn source(&self) -> &S {
        &self.source
    }

    /// Returns the overlay.
    pub const fn overlay(&self) -> &Overlay {
        &self.overlay
    }

    /// Returns the source and the overlay.
    pub fn into_parts(self) -> (S, Overlay) {
        (self.source, self.overlay)
    }
}

impl<S: ReadAt> ReadAt for Recovered<S> {
    async fn read_at<B: IoBufMut>(&self, mut buf: B, off: u64) -> BufResult<usize, B> {
        if self.overlay.is_empty() {
            return self.source.read_at(buf, off).await;
        }
        // Read, lay the overlay over what was read, and hand it on.
        let BufResult(result, mut read) = self
            .source
            .read_at(Vec::with_capacity(buf.spare()), off)
            .await;
        if let Err(error) = result {
            return BufResult(Err(error), buf);
        }
        self.overlay.lay(off, &mut read);
        let result = Ok(buf.fill_from(&read));
        BufResult(result, buf)
    }

    fn size(&self) -> impl Future<Output = filer_api::Result<u64>> + Send {
        self.source.size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn puts_whole_chunks_only() {
        let mut overlay = Overlay::new();
        assert!(!overlay.put(64, &[1; 128]));
        assert!(!overlay.put(0, &[1; 100]));
        assert!(!overlay.put(u64::MAX - 127, &[1; 128]));
        assert!(overlay.is_empty());
        assert!(overlay.put(0, &[]));
        assert!(overlay.put(256, &[1; 256]));
        assert!(overlay.put(384, &[2; 128]));
        let mut out = vec![0; 300];
        overlay.lay(200, &mut out);
        assert_eq!(&out[..56], &[0; 56]);
        assert_eq!(&out[56..184], &[1; 128]);
        assert_eq!(&out[184..], &[2; 116]);
    }
}
