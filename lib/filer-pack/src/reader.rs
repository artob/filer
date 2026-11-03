// This is free and unencumbered software released into the public domain.

use crate::Parser;
use alloc::vec::Vec;
use filer_api::{Error, ErrorKind, ReadAt};

/// The least a [`Reader`] reads ahead.
const MIN_AHEAD: usize = 512;

/// The most a [`Reader`] reads ahead, and what it starts with, a quarter of
/// that.
const MAX_AHEAD: usize = 64 * 1024;

/// A reader of an archive's headers that reads ahead, so that headers
/// lying close together cost a fraction of a source read each.
///
/// A read within what the reader holds costs no source read. Any other
/// reads the bytes asked for and some after them, as much as the last
/// reads suggest: the read-ahead doubles, up to 64 KiB, while what it
/// read serves later reads, as headers of small members do, and shrinks
/// to a quarter, down to 512 bytes, when it served none, as when large
/// members lie between headers. Reads may jump anywhere before the end,
/// forwards or back.
///
/// Through a driver whose reads each cost a thread hop, opening an archive
/// of many small members this way takes a fraction of the time it takes
/// with a read per header (`DESIGN.md` §17.7).
///
/// # Examples
///
/// ```
/// use filer_api::ErrorKind;
/// use filer_pack::{Parser, Reader};
///
/// let archive: &'static [u8] = b"header1;header2;";
/// let mut reader = Reader::new(&archive, Parser::new("example"), 16);
/// assert_eq!(filer_spec::block_on(reader.read(8, 7))?, b"header2");
/// assert_eq!(filer_spec::block_on(reader.read(0, 7))?, b"header1");
/// let past = filer_spec::block_on(reader.read(10, 7)).unwrap_err();
/// assert_eq!(past.kind(), ErrorKind::Corrupt);
/// # Ok::<(), filer_api::Error>(())
/// ```
#[derive(Debug)]
pub struct Reader<'s, S> {
    source: &'s S,
    parser: Parser,
    /// Where the source ends, which no read passes.
    end: u64,
    /// The offset of `buf`'s first byte.
    start: u64,
    buf: Vec<u8>,
    /// How much a source read reads at least.
    ahead: usize,
    /// The reads `buf` served since it was read.
    hits: usize,
}

impl<'s, S: ReadAt> Reader<'s, S> {
    /// Returns a reader of `source`, which ends at `end`, for the format
    /// `parser` reads, which its errors name.
    #[must_use]
    pub const fn new(source: &'s S, parser: Parser, end: u64) -> Self {
        Self {
            source,
            parser,
            end,
            start: 0,
            buf: Vec::new(),
            ahead: MAX_AHEAD / 4,
            hits: 0,
        }
    }

    /// Returns where the source ends, which no read passes.
    #[must_use]
    pub const fn end(&self) -> u64 {
        self.end
    }

    /// Returns the `len` bytes at `offset`, reading them, and some after,
    /// if the reader does not hold them.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](ErrorKind::Corrupt), naming `offset`, if they pass the
    /// end, as in a truncated archive; otherwise the source's own.
    pub async fn read(&mut self, offset: u64, len: usize) -> Result<&[u8], Error> {
        let parser = self.parser;
        let truncated = || parser.fail(ErrorKind::Corrupt, offset, "truncated");
        let stop = u64::try_from(len)
            .ok()
            .and_then(|len| offset.checked_add(len))
            .filter(|&stop| stop <= self.end)
            .ok_or_else(truncated)?;
        let held = self.start.saturating_add(to_u64(self.buf.len()));
        if offset >= self.start && stop <= held && !self.buf.is_empty() {
            self.hits = self.hits.saturating_add(1);
        } else {
            if !self.buf.is_empty() {
                self.ahead = if self.hits > 0 {
                    self.ahead.saturating_mul(2).min(MAX_AHEAD)
                } else {
                    (self.ahead / 4).max(MIN_AHEAD)
                };
            }
            let wanted = len.max(self.ahead);
            let left = usize::try_from(self.end.saturating_sub(offset)).unwrap_or(usize::MAX);
            self.buf = parser.read(self.source, offset, wanted.min(left)).await?;
            self.start = offset;
            self.hits = 0;
        }
        let from = usize::try_from(offset.saturating_sub(self.start)).map_err(|_| truncated())?;
        let to = from.checked_add(len).ok_or_else(truncated)?;
        self.buf.get(from..to).ok_or_else(truncated)
    }
}

/// Converts a length that fits in memory to a `u64`.
fn to_u64(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{vec, vec::Vec};
    use core::sync::atomic::Ordering;
    use filer_spec::{Counted, block_on};

    /// Returns an archive of `count` headers of 8 bytes, each followed by
    /// `gap` bytes of data, and the reader's reads of every header.
    fn headers(count: usize, gap: usize) -> (Vec<u8>, usize) {
        let step = 8 + gap;
        let mut archive = vec![0; count * step];
        for (i, header) in archive.chunks_mut(step).enumerate() {
            header[..8].copy_from_slice(&(i as u64).to_le_bytes());
        }
        let source = Counted::new(archive.clone());
        let reads = source.reads();
        let mut reader = Reader::new(&source, Parser::new("test"), archive.len() as u64);
        for i in 0..count {
            let header = block_on(reader.read((i * step) as u64, 8)).unwrap();
            assert_eq!(header, (i as u64).to_le_bytes());
        }
        (archive, reads.load(Ordering::Relaxed))
    }

    #[test]
    fn reads_close_headers_together() {
        // 1,000 headers in 16,000 bytes: one read of 16 KiB would do.
        let (_, reads) = headers(1000, 8);
        assert!(reads <= 2, "{reads} reads");
    }

    #[test]
    fn reads_little_past_headers_far_apart() {
        // Headers 100 KiB apart take one read each, soon of 512 bytes.
        let gap = 100 * 1024;
        let (archive, reads) = headers(50, gap);
        assert_eq!(reads, 50);
        let source = Counted::new(archive.clone());
        let mut reader = Reader::new(&source, Parser::new("test"), archive.len() as u64);
        for i in 0..50 {
            block_on(reader.read((i * (gap + 8)) as u64, 8)).unwrap();
            assert!(reader.buf.len() <= MAX_AHEAD / 4, "{}", reader.buf.len());
        }
        assert_eq!(reader.ahead, MIN_AHEAD);
    }

    #[test]
    fn refuses_reads_past_the_end() {
        let archive: &'static [u8] = b"0123456789";
        let mut reader = Reader::new(&archive, Parser::new("test"), 10);
        assert_eq!(block_on(reader.read(4, 6)).unwrap(), b"456789");
        for (offset, len) in [(5, 6), (10, 1), (u64::MAX, 2)] {
            let error = block_on(reader.read(offset, len)).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::Corrupt, "{offset}+{len}");
        }
        assert_eq!(block_on(reader.read(10, 0)).unwrap(), b"");
    }
}
