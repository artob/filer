// This is free and unencumbered software released into the public domain.

use crate::{Decoder, lock::Lock};
use alloc::{boxed::Box, vec, vec::Vec};
use core::ops::Range;
use filer_api::{BufResult, Error, ErrorKind, OpKind, ReadAt};

/// How many streams [`Streams`] keeps.
const KEPT: usize = 4;

/// How much packed input a stream reads from the source at once.
const INPUT_CHUNK: usize = 64 * 1024;

/// How much a stream decodes at once.
pub(crate) const OUTPUT_CHUNK: usize = 32 * 1024;

/// Identifies what a [`Stream`] decodes, among those [`Streams`] keeps:
/// such as a member, by its inode, or a solid block, by its number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StreamId(u64);

impl StreamId {
    /// Returns the id numbered `n`, which only its [`Streams`] interprets.
    #[must_use]
    pub const fn new(n: u64) -> Self {
        Self(n)
    }
}

/// Streams kept part way through what they decode, so that a sequential
/// reader decodes once rather than from the start at every read.
///
/// A read [`take`](Self::take)s the stream furthest along that has not
/// passed its offset, or starts a new one, and [`keep`](Self::keep)s it
/// again after; seeking backwards then starts over. Up to four streams are
/// kept, the least recently kept dropped first. The kept streams are
/// behind a lock, held only while taking or keeping one.
///
/// # Examples
///
/// ```
/// use filer_pack::{Passthrough, Stream, StreamId, Streams};
///
/// let streams = Streams::new();
/// let id = StreamId::new(7);
/// let packed: &'static [u8] = b"stored bytes";
/// let mut stream = streams.take(id, 0).unwrap_or_else(|| Stream::new(Box::new(Passthrough)));
/// let read = stream.read(&packed, 0..12, 12, 0..6);
/// assert_eq!(filer_spec::block_on(read)?, b"stored");
/// streams.keep(id, stream);
///
/// // The next read continues where that one stopped.
/// let mut stream = streams.take(id, 6).unwrap();
/// assert_eq!(stream.decoded(), 6);
/// assert!(streams.take(id, 6).is_none());
/// # Ok::<(), filer_api::Error>(())
/// ```
pub struct Streams {
    kept: Lock<Vec<(StreamId, Stream)>>,
}

impl Default for Streams {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for Streams {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Streams").finish_non_exhaustive()
    }
}

impl Streams {
    /// Returns a cache keeping no streams yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            kept: Lock::new(Vec::new()),
        }
    }

    /// Takes the kept stream of `id` that is furthest along without having
    /// decoded past `off`, if any.
    #[must_use]
    pub fn take(&self, id: StreamId, off: u64) -> Option<Stream> {
        self.kept.with(|kept| {
            let best = kept
                .iter()
                .enumerate()
                .filter(|(_, (kept, stream))| *kept == id && stream.decoded <= off)
                .max_by_key(|(_, (_, stream))| stream.decoded)
                .map(|(i, _)| i)?;
            Some(kept.remove(best).1)
        })
    }

    /// Keeps `stream` of `id` for later reads, dropping the least recently
    /// kept stream if there are too many.
    pub fn keep(&self, id: StreamId, stream: Stream) {
        self.kept.with(|kept| {
            if kept.len() >= KEPT {
                kept.remove(0);
            }
            kept.push((id, stream));
        });
    }

    /// Reads the decoded bytes within `wanted` of `id`, whose packed bytes
    /// lie at `packed` in `source` and which decodes to `size` bytes, as
    /// [`Stream::read`] does: on the kept stream that [`take`](Self::take)
    /// finds, or on a new one decoding with what `decoder` returns, which
    /// is then kept unless it reached the end.
    ///
    /// # Errors
    ///
    /// Those of `decoder` and [`Stream::read`]. A stream that fails is not
    /// kept.
    pub async fn read<S: ReadAt>(
        &self,
        id: StreamId,
        source: &S,
        packed: Range<u64>,
        size: u64,
        wanted: Range<u64>,
        decoder: impl FnOnce() -> Result<Box<dyn Decoder>, Error> + Send,
    ) -> Result<Vec<u8>, Error> {
        let mut stream = match self.take(id, wanted.start) {
            Some(stream) => stream,
            None => Stream::new(decoder()?),
        };
        let data = stream.read(source, packed, size, wanted).await?;
        if stream.decoded < size {
            self.keep(id, stream);
        }
        Ok(data)
    }
}

/// A decoder part way through what it decodes: a member, a compressed
/// archive, or a solid block, whose packed bytes lie in a range of a
/// [`ReadAt`] source.
pub struct Stream {
    /// The decoded bytes produced so far.
    decoded: u64,
    /// The packed bytes read from the source so far, including `input`.
    fetched: u64,
    /// Packed bytes read but not yet all decoded, from `used` on.
    input: Vec<u8>,
    used: usize,
    decoder: Box<dyn Decoder>,
}

impl core::fmt::Debug for Stream {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Stream")
            .field("decoded", &self.decoded)
            .field("fetched", &self.fetched)
            .finish_non_exhaustive()
    }
}

impl Stream {
    /// Returns a stream at the start, decoding with `decoder`.
    #[must_use]
    pub fn new(decoder: Box<dyn Decoder>) -> Self {
        Self {
            decoded: 0,
            fetched: 0,
            input: Vec::new(),
            used: 0,
            decoder,
        }
    }

    /// Returns how many bytes it has decoded so far.
    #[must_use]
    pub const fn decoded(&self) -> u64 {
        self.decoded
    }

    /// Decodes on to the end of `wanted`, from the packed bytes at `packed`
    /// in `source`, and returns the decoded bytes within `wanted`. `size`
    /// is the declared size of the whole, which decoding never passes. On
    /// reaching it, the decoder is given the rest of the packed bytes with
    /// no room for output, so that it reads what follows the data, such as
    /// a trailer with a checksum, until it takes no more; then it
    /// [`finish`](Decoder::finish)es. The stream must not have decoded
    /// past `wanted`'s start.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](ErrorKind::Corrupt) if the packed bytes end early, the
    /// decoder stops making progress or breaks its contract, or the stream
    /// is past `wanted`'s start; the decoder's and the source's errors.
    pub async fn read<S: ReadAt>(
        &mut self,
        source: &S,
        packed: Range<u64>,
        size: u64,
        wanted: Range<u64>,
    ) -> Result<Vec<u8>, Error> {
        let corrupt = || Error::new(ErrorKind::Corrupt, OpKind::Read);
        let (off, end) = (wanted.start, wanted.end.min(size));
        if self.decoded > off {
            return Err(corrupt());
        }
        let capacity = usize::try_from(end.saturating_sub(off)).unwrap_or(0);
        let mut data = Vec::with_capacity(capacity);
        let mut output = vec![0; OUTPUT_CHUNK];
        while self.decoded < end {
            self.fetch(source, &packed).await?;
            // Stop at the end of the read, so that a kept stream can
            // continue from there, and so never pass the declared size.
            let room = usize::try_from(end.saturating_sub(self.decoded))
                .map_or(OUTPUT_CHUNK, |n| n.min(OUTPUT_CHUNK));
            let input = self.input.get(self.used..).unwrap_or_default();
            let room = output.get_mut(..room).ok_or_else(corrupt)?;
            let (consumed, produced) = self.decoder.decode(input, room)?;
            if consumed > input.len() || produced > room.len() {
                // The decoder broke its contract.
                return Err(corrupt());
            }
            if consumed == 0 && produced == 0 {
                // No progress: the packed data ended early.
                return Err(corrupt());
            }
            self.used = self.used.saturating_add(consumed);
            let start = self.decoded;
            let produced = to_u64(produced);
            self.decoded = self.decoded.saturating_add(produced);
            // Keep what falls within `off..end`, which is below `produced`.
            let within = |at: u64| usize::try_from(at.saturating_sub(start).min(produced));
            let (Ok(from), Ok(to)) = (within(off), within(end)) else {
                return Err(corrupt());
            };
            data.extend_from_slice(output.get(from..to).unwrap_or_default());
            if self.decoded == size {
                self.drain(source, &packed).await?;
                self.decoder.finish()?;
            }
        }
        Ok(data)
    }

    /// Reads the next packed bytes from `source` if all those read are
    /// used and any are left.
    async fn fetch<S: ReadAt>(&mut self, source: &S, packed: &Range<u64>) -> Result<(), Error> {
        let packed_len = packed.end.saturating_sub(packed.start);
        if self.used == self.input.len() && self.fetched < packed_len {
            let len = usize::try_from(packed_len.saturating_sub(self.fetched))
                .map_or(INPUT_CHUNK, |n| n.min(INPUT_CHUNK));
            let at = packed
                .start
                .checked_add(self.fetched)
                .ok_or_else(|| Error::new(ErrorKind::Corrupt, OpKind::Read))?;
            self.input = read_exact(source, at, len).await?;
            self.used = 0;
            self.fetched = self.fetched.saturating_add(to_u64(len));
        }
        Ok(())
    }

    /// Gives the decoder the rest of the packed bytes with no room for
    /// output, until it takes no more: past the data, as a trailer.
    async fn drain<S: ReadAt>(&mut self, source: &S, packed: &Range<u64>) -> Result<(), Error> {
        loop {
            self.fetch(source, packed).await?;
            let input = self.input.get(self.used..).unwrap_or_default();
            let (consumed, produced) = self.decoder.decode(input, &mut [])?;
            if consumed > input.len() || produced > 0 {
                // The decoder broke its contract.
                return Err(Error::new(ErrorKind::Corrupt, OpKind::Read));
            }
            if consumed == 0 {
                return Ok(());
            }
            self.used = self.used.saturating_add(consumed);
        }
    }
}

/// Reads exactly `len` bytes at `at` of `source`: an archive that ends
/// first is [`Corrupt`](ErrorKind::Corrupt), as it has been read before.
pub(crate) async fn read_exact<S: ReadAt>(
    source: &S,
    at: u64,
    len: usize,
) -> Result<Vec<u8>, Error> {
    let BufResult(result, data) = source.read_exact_at(Vec::with_capacity(len), at).await;
    result.map_err(|error| match error.kind() {
        ErrorKind::UnexpectedEof => Error::new(ErrorKind::Corrupt, OpKind::Read),
        _ => error,
    })?;
    Ok(data)
}

/// Converts a length that fits in memory to a `u64`.
pub(crate) fn to_u64(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}
