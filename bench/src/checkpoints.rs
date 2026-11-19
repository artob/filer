// This is free and unencumbered software released into the public domain.

//! Checkpoints, as `METHOD.md` asks, of the decoders that offer them:
//! ours, and the counterparts that copy their state, zlib's and zlib-ng's
//! by `inflateCopy`, and `miniz_oxide`'s by `Clone`.
//!
//! `checkpoints` decodes every input at every level with each, given and
//! taken in pieces of `PIECE` bytes, as `filer-pack` reads, and at `PLACES`
//! places spread over what it decodes, at the first call that reaches each
//! where the decoder offers one, takes a checkpoint, as `filer-pack` does
//! every few MiB: timed, taken again and again, as many at a time as take
//! `BATCH`, each dropped untimed, for `TIMED` and `ITERATIONS` times at
//! least; and counted, the heap it holds and its allocations, as `memory`
//! counts. Then it resumes from it, as `filer-pack` does to read there: a
//! copy of it, given the input from where it had consumed to, less what
//! it held of it, decodes its first piece of output, timed likewise, and
//! checked. Its figures are each place's median times.

use crate::{
    Options, Result, corpus,
    decoders::{Decoder, Failure, Resumable},
    machine::Machine,
    memory, procedure,
};
use serde::Serialize;
use std::{
    io::Write as _,
    path::Path,
    time::{Duration, Instant},
};

/// The pieces each decoder is given and taken in.
const PIECE: usize = 32 << 10;

/// The places a checkpoint is taken at, spread over what a stream decodes.
pub const PLACES: usize = 3;

/// The least each is timed for, and the fewest times.
const TIMED: Duration = Duration::from_millis(50);
const ITERATIONS: usize = 10;

/// The least a time lasts: as many are timed at a time as last that long,
/// as one may last less than the timer resolves, or than reading it costs.
const BATCH: Duration = Duration::from_micros(20);

/// A line of the results' file, after the machine's.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Record<'a> {
    Machine(&'a Machine),
    Checkpoint {
        codec: &'a str,
        input: &'a str,
        level: String,
        piece: usize,
        /// The input's size, and packed.
        bytes: usize,
        packed: usize,
        decoder: &'a str,
        /// Where it was taken: how much the decoder had produced, and
        /// consumed, less what it held of it.
        at: usize,
        from: usize,
        /// The heap it holds and its allocations, if counted.
        held: Option<u64>,
        allocations: Option<u64>,
        /// Each time taken to take it, `takes` times, and to resume from
        /// it, `resumes` times, in nanoseconds.
        take: Vec<u64>,
        takes: usize,
        resume: Vec<u64>,
        resumes: usize,
    },
    Failure {
        codec: &'a str,
        input: &'a str,
        level: String,
        piece: usize,
        decoder: &'a str,
        failure: &'a str,
    },
}

/// What was measured at a place: the decoder's position, as `Record`
/// gives it, the checkpoint's heap and allocations, and the times, each of
/// as many as `Timed` gives.
struct Place {
    at: usize,
    from: usize,
    held: u64,
    allocations: u64,
    take: Timed,
    resume: Timed,
}

/// Each time taken, in nanoseconds, of how many at a time.
struct Timed {
    times: Vec<u64>,
    each: usize,
}

/// The `checkpoints` command: measures the checkpoints of every decoder
/// of each codec that offers them, over every input at every level; prints
/// a table of each one's median times to take and to resume from one, in
/// microseconds, and the most heap one held, in KiB, over the places; and
/// writes each place's to `bench/results/`.
pub fn run(options: &Options, bench: &Path) -> Result<()> {
    let machine = Machine::now(bench);
    let (path, mut results) = machine.create(bench)?;
    let mut record = |record: &Record| -> Result<()> {
        writeln!(results, "{}", serde_json::to_string(record)?)?;
        Ok(())
    };
    record(&Record::Machine(&machine))?;
    eprintln!("writing {}", path.display());
    let inputs = corpus::inputs(&options.only)?;
    for &codec in &options.codecs {
        let decoders: Vec<&Decoder> = codec
            .decoders()
            .iter()
            .filter(|decoder| decoder.resumable.is_some())
            .collect();
        println!(
            "{:<26} {:>8} {:<14} {:>10} {:>10} {:>10}",
            codec.name(),
            "level",
            "decoder",
            "take µs",
            "resume µs",
            "held KiB"
        );
        for input in &inputs {
            let original = input.bytes()?;
            for level in codec.levels(original.len()) {
                let packed = input.encoded(codec, level)?;
                for decoder in &decoders {
                    let (name, label) = (&input.name, level.label());
                    print!("{name:<26} {label:>8} {:<14}", decoder.name);
                    let places = match measure(decoder, &packed, &original) {
                        Ok(places) => places,
                        Err(failure) => {
                            println!(" ({failure})");
                            record(&Record::Failure {
                                codec: codec.name(),
                                input: name,
                                level: label,
                                piece: PIECE,
                                decoder: decoder.name,
                                failure: &failure,
                            })?;
                            continue;
                        },
                    };
                    let counted = memory::counts(decoder);
                    for place in &places {
                        record(&Record::Checkpoint {
                            codec: codec.name(),
                            input: name,
                            level: label.clone(),
                            piece: PIECE,
                            bytes: original.len(),
                            packed: packed.len(),
                            decoder: decoder.name,
                            at: place.at,
                            from: place.from,
                            held: counted.then_some(place.held),
                            allocations: counted.then_some(place.allocations),
                            take: place.take.times.clone(),
                            takes: place.take.each,
                            resume: place.resume.times.clone(),
                            resumes: place.resume.each,
                        })?;
                    }
                    summarize(&places, counted);
                }
            }
        }
    }
    eprintln!("wrote {}", path.display());
    Ok(())
}

/// Prints the median over `places` of each one's median times, and the
/// most heap one held, if counted.
fn summarize(places: &[Place], counted: bool) {
    if places.is_empty() {
        println!(" (none taken)");
        return;
    }
    let median = |timed: &dyn Fn(&Place) -> &Timed| {
        let mut medians: Vec<f64> = places
            .iter()
            .map(|place| {
                let Timed { times, each } = timed(place);
                let mut times: Vec<f64> =
                    times.iter().map(|&ns| ns as f64 / *each as f64).collect();
                procedure::median(&mut times)
            })
            .collect();
        procedure::median(&mut medians) / 1e3
    };
    let (take, resume) = (median(&|place| &place.take), median(&|place| &place.resume));
    let held = places.iter().map(|place| place.held).max().unwrap_or(0);
    print!(" {take:>10.3} {resume:>10.3}");
    match counted {
        true => println!(" {:>10.1}", held as f64 / 1024.0),
        false => println!(" {:>10}", "-"),
    }
}

/// Decodes `packed` with `decoder` in pieces, measuring a checkpoint at
/// each place, and checks that it decoded `original`.
fn measure(
    decoder: &Decoder,
    packed: &[u8],
    original: &[u8],
) -> std::result::Result<Vec<Place>, Failure> {
    let resumable = decoder.resumable.ok_or("offers no checkpoints")?;
    let mut decoding = resumable()?;
    let mut output = vec![0; original.len()];
    let (mut read, mut written, mut places) = (0, 0, Vec::new());
    loop {
        let reached = original.len() * (places.len() + 1) / (PLACES + 1);
        // Places past the end, as of a stream decoded in one piece, are
        // none.
        let within = written < original.len();
        if places.len() < PLACES
            && written >= reached.max(1)
            && within
            && let Some(place) = place(&*decoding, packed, original, read, written)?
        {
            places.push(place);
        }
        let into = packed.len().min(read + PIECE);
        let room = output.len().min(written + PIECE);
        let (consumed, produced) =
            decoding.decode(&packed[read..into], &mut output[written..room])?;
        (read, written) = (read + consumed, written + produced);
        if (consumed, produced) == (0, 0) {
            break;
        }
    }
    match output[..written] == *original {
        true => Ok(places),
        false => Err("wrong".into()),
    }
}

/// Measures a checkpoint of `decoding`, which has consumed `read` bytes of
/// `packed` and produced `written` of `original`, if it offers one.
fn place(
    decoding: &dyn Resumable,
    packed: &[u8],
    original: &[u8],
    read: usize,
    written: usize,
) -> std::result::Result<Option<Place>, Failure> {
    let (kept, held, allocations) = memory::held(|| decoding.checkpoint());
    let Some(kept) = kept else {
        return Ok(None);
    };
    let from = read
        .checked_sub(decoding.buffered())
        .ok_or("buffered more than it consumed")?;
    // Each batch's checkpoints, and resumed decoders, are kept until it is
    // timed, and dropped untimed.
    let take = timed(|each| {
        let mut taken = Vec::with_capacity(each);
        let start = Instant::now();
        for _ in 0..each {
            taken.push(decoding.checkpoint());
        }
        let elapsed = start.elapsed();
        drop(taken);
        Ok(elapsed)
    })?;
    // Its first piece of output, or what is left.
    let first = &original[written..original.len().min(written + PIECE)];
    let mut output = vec![0; first.len()];
    let resume = timed(|each| {
        output.fill(0);
        let mut resumed = Vec::with_capacity(each);
        let start = Instant::now();
        for _ in 0..each {
            let mut decoding = kept.checkpoint().ok_or("offered no copy of itself")?;
            let (mut read, mut produced) = (from, 0);
            while produced < output.len() {
                let into = packed.len().min(read + PIECE);
                let (consumed, more) =
                    decoding.decode(&packed[read..into], &mut output[produced..])?;
                (read, produced) = (read + consumed, produced + more);
                if (consumed, more) == (0, 0) {
                    break;
                }
            }
            resumed.push(decoding);
        }
        let elapsed = start.elapsed();
        drop(resumed);
        match output == first {
            true => Ok(elapsed),
            false => Err("resumed wrong".into()),
        }
    })?;
    Ok(Some(Place {
        at: written,
        from,
        held,
        allocations,
        take,
        resume,
    }))
}

/// Each time `batch` takes to do as many at a time as last `BATCH`, by
/// the least time one took of a few, run until `TIMED` and `ITERATIONS`
/// times at least.
fn timed(
    mut batch: impl FnMut(usize) -> std::result::Result<Duration, Failure>,
) -> std::result::Result<Timed, Failure> {
    let mut once = Duration::MAX;
    for _ in 0..3 {
        once = once.min(batch(1)?);
    }
    let once = once.as_nanos().max(1);
    let each = usize::try_from(BATCH.as_nanos().div_ceil(once)).unwrap_or(1);
    let (mut times, mut total) = (Vec::new(), Duration::ZERO);
    while times.len() < ITERATIONS || total < TIMED {
        let elapsed = batch(each)?;
        total += elapsed;
        times.push(elapsed.as_nanos() as u64);
    }
    Ok(Timed { times, each })
}
