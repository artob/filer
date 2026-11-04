// This is free and unencumbered software released into the public domain.

use crate::{
    Codec, Stream, StreamId, Streams,
    streams::{read_exact, to_u64},
};
use core::ops::Range;
use filer_api::{BufResult, Error, ErrorKind, IoBufMut, OpKind, ReadAt};

/// The one id an [`Unpacked`] keeps its streams under.
const WHOLE: StreamId = StreamId::new(0);

/// A [`ReadAt`] source over the decoded bytes of a packed range of another
/// source: a compressed tarball or initramfs image, or a solid block of
/// members, which an archive driver then reads as if it were stored.
///
/// A read continues a decoding stream that has not passed its offset, or
/// starts a new one with the codec's decoder, and keeps it after, through
/// [`Streams`]: reading sequentially decodes once, and seeking backwards
/// decodes again from the start. Decoding never passes the declared size,
/// which so bounds what a decompression bomb can produce, and the codec's
/// check, such as a checksum, runs once a read reaches it. A stored codec
/// reads the packed range as is.
///
/// # Examples
///
/// ```
/// use filer_api::{BufResult, Error, ReadAt};
/// use filer_pack::{Codec, Decoder, Passthrough, Unpacked};
///
/// /// A codec whose bytes are packed as is, but decoded as a stream.
/// #[derive(Clone, Copy)]
/// struct Copied;
///
/// impl Codec for Copied {
///     fn is_stored(&self) -> bool {
///         false
///     }
///
///     fn decoder(&self) -> Result<Box<dyn Decoder>, Error> {
///         Ok(Box::new(Passthrough))
///     }
/// }
///
/// // Five bytes of a header, then the packed whole.
/// let source: &'static [u8] = b"head:payload";
/// let unpacked = Unpacked::new(source, 5..12, 7, Copied);
/// let read = unpacked.read_exact_at(Vec::with_capacity(4), 3);
/// let BufResult(result, buf) = filer_spec::block_on(read);
/// result?;
/// assert_eq!(buf, b"load");
/// assert_eq!(filer_spec::block_on(unpacked.size())?, 7);
/// # Ok::<(), filer_api::Error>(())
/// ```
#[derive(Debug)]
pub struct Unpacked<S, C> {
    source: S,
    packed: Range<u64>,
    size: u64,
    codec: C,
    streams: Streams,
}

impl<S: ReadAt, C: Codec> Unpacked<S, C> {
    /// Returns the bytes that `packed`, a range of `source`, decodes to with
    /// `codec`: `size` bytes, as declared.
    #[must_use]
    pub fn new(source: S, packed: Range<u64>, size: u64, codec: C) -> Self {
        Self {
            source,
            packed,
            size,
            codec,
            streams: Streams::new(),
        }
    }

    /// Returns the bytes that `packed`, a range of `source`, decodes to with
    /// `codec`, however many they are, up to `max`: decoding them once to
    /// learn how many, checked as the codec checks them, as a compressed
    /// tarball needs, whose stream records no size.
    ///
    /// # Errors
    ///
    /// [`LimitExceeded`](ErrorKind::LimitExceeded) if they pass `max`, as
    /// a decompression bomb would; [`Corrupt`](ErrorKind::Corrupt) if they
    /// fail the codec's check; the codec's, the decoder's, and the
    /// source's.
    pub async fn measured(
        source: S,
        packed: Range<u64>,
        codec: C,
        max: u64,
    ) -> Result<Self, Error> {
        let mut stream = Stream::new(codec.decoder()?);
        let size = stream.measure(&source, packed.clone(), max).await?;
        let unpacked = Self::new(source, packed, size, codec);
        // What reads come back to, if the decoder offered any.
        unpacked.streams.record(WHOLE, &mut stream);
        Ok(unpacked)
    }

    /// Returns the source of the packed bytes.
    #[must_use]
    pub const fn source(&self) -> &S {
        &self.source
    }

    /// Returns the codec that decodes them.
    #[must_use]
    pub const fn codec(&self) -> C {
        self.codec
    }

    /// Reads at `off` into the spare capacity of `buf`, never past the
    /// declared size.
    async fn read<B: IoBufMut>(&self, buf: &mut B, off: u64) -> Result<usize, Error> {
        let remaining = self.size.saturating_sub(off);
        let wanted = usize::try_from(remaining).map_or(buf.spare(), |r| r.min(buf.spare()));
        if wanted == 0 {
            return Ok(0);
        }
        let end = off.saturating_add(to_u64(wanted));
        let data = if self.codec.is_stored() {
            let corrupt = || Error::new(ErrorKind::Corrupt, OpKind::Read);
            // Stored bytes are the packed range's, which they must not pass.
            let packed_len = self.packed.end.saturating_sub(self.packed.start);
            if end > packed_len {
                return Err(corrupt());
            }
            let at = self.packed.start.checked_add(off).ok_or_else(corrupt)?;
            read_exact(&self.source, at, wanted).await?
        } else {
            let decoder = || self.codec.decoder();
            let (packed, wanted) = (self.packed.clone(), off..end);
            self.streams
                .read(WHOLE, &self.source, packed, self.size, wanted, decoder)
                .await?
        };
        Ok(buf.fill_from(&data))
    }
}

impl<S: ReadAt, C: Codec> ReadAt for Unpacked<S, C> {
    async fn read_at<B: IoBufMut>(&self, mut buf: B, off: u64) -> BufResult<usize, B> {
        let result = self.read(&mut buf, off).await;
        BufResult(result, buf)
    }

    async fn size(&self) -> Result<u64, Error> {
        Ok(self.size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Decoder, Stored, streams::OUTPUT_CHUNK, toy::Doubled};
    use alloc::{boxed::Box, vec, vec::Vec};
    use core::sync::atomic::{AtomicUsize, Ordering};
    use filer_spec::{block_on, fuzz::Rng};

    /// Decoders started by [`Counted`], which only one test uses.
    static COUNTED: AtomicUsize = AtomicUsize::new(0);

    /// The toy codec, counting the decoders it starts.
    #[derive(Clone, Copy)]
    struct Counted(Doubled);

    impl Codec for Counted {
        fn is_stored(&self) -> bool {
            false
        }

        fn decoder(&self) -> Result<Box<dyn Decoder>, Error> {
            COUNTED.fetch_add(1, Ordering::Relaxed);
            self.0.decoder()
        }
    }

    /// Returns packed bytes after a header of `at` bytes, as many as take
    /// several output chunks, and what the toy codec decodes them to.
    fn packed(at: usize) -> (&'static [u8], Vec<u8>) {
        let mut rng = Rng::new(0x5EED);
        let mut source = vec![0xEE; at];
        source.extend((0..3 * OUTPUT_CHUNK).map(|_| rng.below(256) as u8));
        let decoded = source[at..].iter().flat_map(|&b| [b, b]).collect();
        (Box::leak(source.into_boxed_slice()), decoded)
    }

    fn sum(bytes: &[u8]) -> u64 {
        bytes.iter().map(|&b| u64::from(b)).sum()
    }

    fn read_at<S: ReadAt>(source: &S, off: u64, len: usize) -> Result<Vec<u8>, Error> {
        let BufResult(result, buf) = block_on(source.read_at(Vec::with_capacity(len), off));
        result.map(|_| buf)
    }

    #[test]
    fn reads_at_random_offsets_as_a_decoded_copy() {
        let (source, decoded) = packed(100);
        let range = 100..source.len() as u64;
        let codec = Doubled { sum: sum(&decoded) };
        let unpacked = Unpacked::new(source, range, decoded.len() as u64, codec);
        assert_eq!(block_on(unpacked.size()).unwrap(), decoded.len() as u64);
        let mut rng = Rng::new(7);
        for _ in 0..200 {
            let off = rng.below(decoded.len() as u64 + 10) as usize;
            let len = rng.below(3 * OUTPUT_CHUNK as u64) as usize;
            let read = read_at(&unpacked, off as u64, len).unwrap();
            let expected = decoded.get(off..).unwrap_or_default();
            assert_eq!(read, expected[..len.min(expected.len())], "{off}+{len}");
        }
        // The checksum fails a read that reaches the end.
        let wrong = Unpacked::new(
            source,
            100..source.len() as u64,
            decoded.len() as u64,
            Doubled { sum: 0 },
        );
        let error = read_at(&wrong, 0, decoded.len()).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt);
    }

    #[test]
    fn measures_what_it_decodes_to() {
        let (source, decoded) = packed(100);
        let range = 100..source.len() as u64;
        let len = decoded.len() as u64;
        let codec = Doubled { sum: sum(&decoded) };
        let measured = block_on(Unpacked::measured(source, range.clone(), codec, len)).unwrap();
        assert_eq!(block_on(measured.size()).unwrap(), len);
        assert_eq!(read_at(&measured, 0, decoded.len()).unwrap(), decoded);
        // Past the most it may decode, or failing the codec's check.
        let error =
            block_on(Unpacked::measured(source, range.clone(), codec, len - 1)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::LimitExceeded);
        let error =
            block_on(Unpacked::measured(source, range, Doubled { sum: 0 }, len)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt);
    }

    #[test]
    fn resumes_from_checkpoints_when_reading_back() {
        // A MiB packed, 16 reads of the source, decoding to 32 intervals.
        let mut rng = Rng::new(0xC4EC);
        let source: Vec<u8> = (0..1 << 20).map(|_| rng.below(256) as u8).collect();
        let decoded: Vec<u8> = source.iter().flat_map(|&b| [b, b]).collect();
        let source = filer_spec::Counted::new(source);
        let reads = source.reads();
        let (range, len) = (0..1 << 20, decoded.len() as u64);
        let codec = Doubled { sum: sum(&decoded) };
        let unpacked = block_on(Unpacked::measured(source, range, codec, len)).unwrap();
        // Near the end, back to the middle, and back to the start, each
        // from the checkpoint before it: a read of the source or two.
        let n = decoded.len();
        for off in [n - 100, n / 2 + 12_345, 1_000, n - 50_000] {
            let before = reads.load(Ordering::Relaxed);
            assert_eq!(
                read_at(&unpacked, off as u64, 100).unwrap(),
                decoded[off..off + 100]
            );
            let reads = reads.load(Ordering::Relaxed) - before;
            assert!(reads <= 2, "{reads} reads at {off}");
        }
    }

    #[test]
    fn decodes_once_when_read_sequentially() {
        let (source, decoded) = packed(0);
        let codec = Counted(Doubled { sum: sum(&decoded) });
        let unpacked = Unpacked::new(source, 0..source.len() as u64, decoded.len() as u64, codec);
        let mut read = Vec::new();
        loop {
            let chunk = read_at(&unpacked, read.len() as u64, 1000).unwrap();
            if chunk.is_empty() {
                break;
            }
            read.extend(chunk);
        }
        assert_eq!(read, decoded);
        assert_eq!(COUNTED.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn reads_stored_bytes_within_the_range() {
        let source: &'static [u8] = b"head:payload:tail";
        let unpacked = Unpacked::new(source, 5..12, 7, Stored);
        assert_eq!(read_at(&unpacked, 3, 100).unwrap(), b"load");
        assert_eq!(read_at(&unpacked, 7, 1).unwrap(), b"");
        // A declared size past the range is refused, not read past it.
        let long = Unpacked::new(source, 5..12, 9, Stored);
        assert_eq!(read_at(&long, 6, 3).unwrap_err().kind(), ErrorKind::Corrupt);
    }
}
