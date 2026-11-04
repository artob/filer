// This is free and unencumbered software released into the public domain.

use crate::{Decoder, lock::Lock};
use alloc::{boxed::Box, collections::BTreeMap, vec, vec::Vec};
use core::ops::Range;
use filer_api::{BufResult, Error, ErrorKind, OpKind, ReadAt};

/// How many streams [`Streams`] keeps.
const KEPT: usize = 4;

/// How much packed input a stream reads from the source at once.
const INPUT_CHUNK: usize = 64 * 1024;

/// How much a stream decodes at once.
pub(crate) const OUTPUT_CHUNK: usize = 32 * 1024;

/// How far apart, in decoded bytes, a stream records checkpoints at the
/// closest: what a read decodes at most before reaching what it wants,
/// once it can resume from the last.
pub(crate) const INTERVAL: u64 = if cfg!(test) { 64 * 1024 } else { 4 << 20 };

/// How much of what it decoded last a stream holds, at least, so that a
/// read a little behind it, as of an archive's files read in reverse,
/// takes it from there rather than decoding again.
const TAIL: usize = if cfg!(test) { 64 * 1024 } else { 4 << 20 };

/// The most checkpoints kept, past which every other one is dropped, as
/// those of gzip each hold a copy of its state, of about 43 KiB.
const MAX_CHECKPOINTS: usize = 256;

/// The most memory the checkpoints kept may hold, by their decoders'
/// [`footprint`](Decoder::footprint)s, past which every other one is
/// dropped too: as those within a zstd frame each hold a copy of its
/// window, of 4 MiB in Ubuntu's initrd images.
const MAX_HELD: usize = 64 << 20;

/// The most memory one checkpoint may hold, so that a few are kept.
const MAX_HELD_ONE: usize = MAX_HELD / 8;

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
/// reader decodes once rather than from the start at every read, and the
/// checkpoints they passed, so that one reading back decodes again only
/// from the nearest.
///
/// A read [`take`](Self::take)s the stream that holds its offset in what
/// it decoded last, or else the stream furthest along that has not passed
/// it, or starts a new one, and [`keep`](Self::keep)s it again after. Up
/// to four streams are kept, the least recently kept dropped first, each
/// holding the last 4 MiB or so it decoded. A read that no kept stream is
/// near starts from the
/// nearest checkpoint before its offset, where a decoder offered one (see
/// [`Decoder::checkpoint`]), or else from the start. The kept streams and
/// checkpoints are behind locks, held only while taking or keeping one.
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
    /// Each stream's checkpoints, by where they lie in what it decodes.
    checkpoints: Lock<BTreeMap<(StreamId, u64), Checkpoint>>,
}

/// Where a stream may be decoded on from: the packed bytes consumed by
/// then, and a decoder that goes on from there.
struct Checkpoint {
    packed: u64,
    decoder: Box<dyn Decoder>,
    /// The decoder's footprint.
    held: usize,
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
            checkpoints: Lock::new(BTreeMap::new()),
        }
    }

    /// Takes the kept stream of `id` that holds `off` in what it decoded
    /// last, or else that is furthest along without having decoded past
    /// it, if any.
    #[must_use]
    pub fn take(&self, id: StreamId, off: u64) -> Option<Stream> {
        self.kept.with(|kept| {
            let best = kept
                .iter()
                .enumerate()
                .filter(|(_, (kept, stream))| *kept == id && stream.held() <= off)
                .min_by_key(|(_, (_, stream))| off.saturating_sub(stream.decoded))
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

    /// Returns a stream of `id` resumed from its checkpoint nearest before
    /// `off`, if it has one past `from`.
    fn resume(&self, id: StreamId, off: u64, from: u64) -> Option<Stream> {
        self.checkpoints.with(|checkpoints| {
            let ((_, decoded), checkpoint) = checkpoints.range((id, 0)..=(id, off)).next_back()?;
            (*decoded > from).then(|| Stream::resume(*decoded, checkpoint))?
        })
    }

    /// Keeps the checkpoints `stream` of `id` recorded, but for those it
    /// has already.
    pub(crate) fn record(&self, id: StreamId, stream: &mut Stream) {
        let recorded = core::mem::take(&mut stream.recorded);
        if recorded.is_empty() {
            return;
        }
        self.checkpoints.with(|checkpoints| {
            for (decoded, checkpoint) in recorded {
                checkpoints.entry((id, decoded)).or_insert(checkpoint);
            }
            // Twice as far apart, rather than ever more of them.
            let held = |checkpoints: &BTreeMap<_, Checkpoint>| {
                checkpoints.values().fold(0_usize, |held, checkpoint| {
                    held.saturating_add(checkpoint.held)
                })
            };
            while checkpoints.len() > 1
                && (checkpoints.len() > MAX_CHECKPOINTS || held(checkpoints) > MAX_HELD)
            {
                let mut kept = false;
                checkpoints.retain(|_, _| {
                    kept = !kept;
                    kept
                });
            }
        });
    }

    /// Returns how many checkpoints it keeps.
    #[cfg(test)]
    pub(crate) fn checkpoints(&self) -> usize {
        self.checkpoints.with(|checkpoints| checkpoints.len())
    }

    /// Reads the decoded bytes within `wanted` of `id`, whose packed bytes
    /// lie at `packed` in `source` and which decodes to `size` bytes, as
    /// [`Stream::read`] does: on the kept stream that [`take`](Self::take)
    /// finds, from a checkpoint nearer, or on a new one decoding with what
    /// `decoder` returns, which is then kept.
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
        let kept = self.take(id, wanted.start);
        let from = kept
            .as_ref()
            .map_or(0, |stream| stream.decoded.min(wanted.start));
        let mut stream = match (self.resume(id, wanted.start, from), kept) {
            (Some(resumed), kept) => {
                if let Some(kept) = kept {
                    self.keep(id, kept);
                }
                resumed
            },
            (None, Some(kept)) => kept,
            (None, None) => Stream::new(decoder()?),
        };
        let data = stream.read(source, packed, size, wanted).await?;
        self.record(id, &mut stream);
        self.keep(id, stream);
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
    /// Where it last recorded a checkpoint, or started.
    since: u64,
    /// The checkpoints it recorded, by where they lie, for [`Streams`].
    recorded: Vec<(u64, Checkpoint)>,
    /// The last of what it decoded, `TAIL` bytes or more.
    tail: Vec<u8>,
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
            since: 0,
            recorded: Vec::new(),
            tail: Vec::new(),
        }
    }

    /// Returns a stream at `checkpoint`, which lies at `decoded`, if its
    /// decoder offers a copy of itself, as it must.
    fn resume(decoded: u64, checkpoint: &Checkpoint) -> Option<Self> {
        Some(Self {
            decoded,
            fetched: checkpoint.packed,
            input: Vec::new(),
            used: 0,
            decoder: checkpoint.decoder.checkpoint()?,
            since: decoded,
            recorded: Vec::new(),
            tail: Vec::new(),
        })
    }

    /// Records a checkpoint where it is, if its decoder offers one, not
    /// too large, and it has decoded far enough since the last.
    fn offer(&mut self) {
        if self.decoded.saturating_sub(self.since) < INTERVAL {
            return;
        }
        // A decoder holding twice what a checkpoint may is not copied.
        if self.decoder.footprint() > MAX_HELD_ONE.saturating_mul(2) {
            return;
        }
        if let Some(decoder) = self.decoder.checkpoint() {
            // What it read but did not pass on, and what the decoder holds.
            let pending = self.input.len().saturating_sub(self.used);
            let pending = to_u64(pending.saturating_add(self.decoder.buffered()));
            let packed = self.fetched.saturating_sub(pending);
            let held = decoder.footprint();
            if held <= MAX_HELD_ONE {
                let checkpoint = Checkpoint {
                    packed,
                    decoder,
                    held,
                };
                self.recorded.push((self.decoded, checkpoint));
            }
            self.since = self.decoded;
        }
    }

    /// Returns how many bytes it has decoded so far.
    #[must_use]
    pub const fn decoded(&self) -> u64 {
        self.decoded
    }

    /// Returns where what it holds of what it decoded last starts.
    fn held(&self) -> u64 {
        self.decoded.saturating_sub(to_u64(self.tail.len()))
    }

    /// Holds `produced`, what it decoded last, dropping what is older
    /// than `TAIL` bytes once there is as much again, so that each byte
    /// moves once at most.
    fn hold(&mut self, produced: &[u8]) {
        if self.tail.len() >= TAIL.saturating_mul(2) {
            self.tail.drain(..self.tail.len().saturating_sub(TAIL));
        }
        self.tail.extend_from_slice(produced);
    }

    /// Decodes on to the end of `wanted`, from the packed bytes at `packed`
    /// in `source`, and returns the decoded bytes within `wanted`. `size`
    /// is the declared size of the whole, which decoding never passes. On
    /// reaching it, the decoder is given the rest of the packed bytes with
    /// no room for output, so that it reads what follows the data, such as
    /// a trailer with a checksum, until it takes no more; then it
    /// [`finish`](Decoder::finish)es. What of `wanted` it decoded already
    /// comes from what it holds of what it decoded last, as long as it
    /// holds `wanted`'s start.
    ///
    /// # Errors
    ///
    /// [`Corrupt`](ErrorKind::Corrupt) if the packed bytes end early, the
    /// decoder stops making progress or breaks its contract, or the stream
    /// is past `wanted`'s start and no longer holds it; the decoder's and
    /// the source's errors.
    pub async fn read<S: ReadAt>(
        &mut self,
        source: &S,
        packed: Range<u64>,
        size: u64,
        wanted: Range<u64>,
    ) -> Result<Vec<u8>, Error> {
        let corrupt = || Error::new(ErrorKind::Corrupt, OpKind::Read);
        let (off, end) = (wanted.start, wanted.end.min(size));
        let held = self.held();
        if off < held {
            return Err(corrupt());
        }
        let capacity = usize::try_from(end.saturating_sub(off)).unwrap_or(0);
        let mut data = Vec::with_capacity(capacity);
        if off < self.decoded {
            let at = |to: u64| usize::try_from(to.min(self.decoded).saturating_sub(held));
            let (Ok(from), Ok(to)) = (at(off), at(end)) else {
                return Err(corrupt());
            };
            data.extend_from_slice(self.tail.get(from..to).unwrap_or_default());
        }
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
            let produced = output.get(..produced).unwrap_or_default();
            let len = to_u64(produced.len());
            self.decoded = self.decoded.saturating_add(len);
            self.offer();
            // Keep what falls within `off..end`, which is below `len`.
            let within = |at: u64| usize::try_from(at.saturating_sub(start).min(len));
            let (Ok(from), Ok(to)) = (within(off), within(end)) else {
                return Err(corrupt());
            };
            data.extend_from_slice(produced.get(from..to).unwrap_or_default());
            self.hold(produced);
            if self.decoded == size {
                self.drain(source, &packed).await?;
                self.decoder.finish()?;
            }
        }
        Ok(data)
    }

    /// Decodes on to the end of what the decoder decodes, from the packed
    /// bytes at `packed` in `source`, and returns how many bytes it has
    /// decoded in all, which must not pass `max`; then the decoder
    /// [`finish`](Decoder::finish)es. Its end is where it makes no progress,
    /// at the end of the packed bytes or before what it declines to read,
    /// such as what follows the last of gzip's members.
    ///
    /// # Errors
    ///
    /// [`LimitExceeded`](ErrorKind::LimitExceeded) if it would decode more
    /// than `max` bytes; the decoder's and the source's.
    pub async fn measure<S: ReadAt>(
        &mut self,
        source: &S,
        packed: Range<u64>,
        max: u64,
    ) -> Result<u64, Error> {
        let mut output = vec![0; OUTPUT_CHUNK];
        loop {
            self.fetch(source, &packed).await?;
            let input = self.input.get(self.used..).unwrap_or_default();
            let (consumed, produced) = self.decoder.decode(input, &mut output)?;
            if consumed > input.len() || produced > output.len() {
                // The decoder broke its contract.
                return Err(Error::new(ErrorKind::Corrupt, OpKind::Read));
            }
            if consumed == 0 && produced == 0 {
                break;
            }
            self.used = self.used.saturating_add(consumed);
            self.decoded = self.decoded.saturating_add(to_u64(produced));
            self.offer();
            self.hold(output.get(..produced).unwrap_or_default());
            if self.decoded > max {
                return Err(Error::new(ErrorKind::LimitExceeded, OpKind::Read));
            }
        }
        self.decoder.finish()?;
        Ok(self.decoded)
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
