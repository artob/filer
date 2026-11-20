// This is free and unencumbered software released into the public domain.

//! Checkpoints, as `METHOD.md` asks, of the decoders that offer them:
//! ours, and the counterparts that copy their state, zlib's and zlib-ng's
//! by `inflateCopy`, and `miniz_oxide`'s by `Clone`.
//!
//! `checkpoints` measures every input at every level by the procedure of
//! `procedure`, in processes of their own, `checkpoints-sample`, each of
//! which decodes it with each decoder in turn, in an order shuffled for
//! it, given and taken in pieces of `PIECE` bytes, as `filer-pack` reads,
//! and at `PLACES` places spread over what it decodes, at the first call
//! that reaches each where the decoder offers one, takes a checkpoint, as
//! `filer-pack` does every few MiB: timed, taken again and again, as many
//! at a time as take `BATCH`, each dropped untimed, for `TIMED` and
//! `ITERATIONS` times at least; and counted, the heap it holds and its
//! allocations, as `memory` counts. Then it resumes from it, as
//! `filer-pack` does to read there: a copy of it, given the input from
//! where it had consumed to, less what it held of it, decodes its first
//! piece of output, timed likewise, and checked. A decoder's figures are,
//! of each process, the median over its places of each's median time,
//! and across processes, as `run`'s, the median of those, with a 95%
//! bootstrap interval, processes started until every interval is narrow
//! enough, or the budget is spent: its times vary from one process to the
//! next more than within one.

use crate::{
    Options, Result,
    corpus::{self, Input},
    decoders::{Codec, Decoder, Failure, Resumable},
    inputs::{Level, SplitMix},
    machine::Machine,
    memory, procedure,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
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

/// A checkpoint taken at a place, as a `checkpoints-sample` process prints
/// it, and the results record it.
#[derive(Serialize, Deserialize)]
struct Taken {
    decoder: String,
    /// Where it was taken: how much the decoder had produced, and
    /// consumed, less what it held of it.
    at: usize,
    from: usize,
    /// The heap it holds and its allocations, if counted.
    held: Option<u64>,
    allocations: Option<u64>,
    /// Each time taken to take it, `takes` times, and to resume from it,
    /// `resumes` times, in nanoseconds.
    take: Vec<u64>,
    takes: usize,
    resume: Vec<u64>,
    resumes: usize,
}

/// What a `checkpoints-sample` process prints, a line of JSON each: a
/// checkpoint taken at a place, or why a decoder failed.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Line {
    Checkpoint(Taken),
    Failure { decoder: String, failure: String },
}

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
        process: usize,
        #[serde(flatten)]
        taken: &'a Taken,
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

/// What was measured at a place: the decoder's position, as `Taken`
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

/// What was measured of a decoder over an input at a level: of each
/// process, the median over its places of each's median time to take a
/// checkpoint, and to resume from one, in nanoseconds; the most heap one
/// held, if counted; and why it failed, if it did.
#[derive(Default)]
struct Measured {
    take: Vec<f64>,
    resume: Vec<f64>,
    held: Option<u64>,
    failure: Option<String>,
}

/// The decoders of `codec` that offer checkpoints.
fn resumable(codec: Codec) -> Vec<&'static Decoder> {
    let decoders = codec.decoders().iter();
    decoders
        .filter(|decoder| decoder.resumable.is_some())
        .collect()
}

/// The `checkpoints-sample` command, in a process of its own: measures
/// the checkpoints of every decoder of `codec` that offers them, over
/// `input` encoded at `level`, in an order shuffled from `seed`, and
/// prints each place's, or why a decoder failed.
pub fn sample(codec: Codec, input: &str, level: Level, seed: u64) -> Result<()> {
    let input = corpus::inputs(input)?
        .into_iter()
        .find(|candidate| candidate.name == input)
        .ok_or("no such input")?;
    let (original, packed) = (input.bytes()?, input.encoded(codec, level)?);
    let decoders = resumable(codec);
    let mut order: Vec<usize> = (0..decoders.len()).collect();
    SplitMix(seed).shuffle(&mut order);
    let mut stdout = std::io::stdout().lock();
    let mut print = |line: &Line| -> Result<()> {
        writeln!(stdout, "{}", serde_json::to_string(line)?)?;
        Ok(())
    };
    for &i in &order {
        let decoder = decoders[i];
        let places = match places(decoder, &packed, &original) {
            Ok(places) => places,
            Err(failure) => {
                print(&Line::Failure {
                    decoder: decoder.name.into(),
                    failure,
                })?;
                continue;
            },
        };
        let counted = memory::counts(decoder);
        for place in places {
            print(&Line::Checkpoint(Taken {
                decoder: decoder.name.into(),
                at: place.at,
                from: place.from,
                held: counted.then_some(place.held),
                allocations: counted.then_some(place.allocations),
                take: place.take.times,
                takes: place.take.each,
                resume: place.resume.times,
                resumes: place.resume.each,
            }))?;
        }
    }
    Ok(())
}

/// The `checkpoints` command: measures the checkpoints of every decoder
/// of each codec that offers them, over every input at every level; prints
/// a table of each one's times to take and to resume from one, in
/// microseconds, each with its interval, and the most heap one held, in
/// KiB; and writes each place's to `bench/results/`.
pub fn run(options: &Options, bench: &Path) -> Result<()> {
    let machine = Machine::now(bench);
    let (path, mut results) = machine.create(bench)?;
    writeln!(
        results,
        "{}",
        serde_json::to_string(&Record::Machine(&machine))?
    )?;
    eprintln!("writing {}", path.display());
    let inputs = corpus::inputs(&options.only)?;
    let mut random = SplitMix(0x5EED);
    for &codec in &options.codecs {
        let decoders = resumable(codec);
        println!(
            "{:<26} {:>8} {:<14} {:>17} {:>17} {:>10}",
            codec.name(),
            "level",
            "decoder",
            "take µs",
            "resume µs",
            "held KiB"
        );
        for input in &inputs {
            for level in codec.levels(input.size()?) {
                let all = measure(codec, input, level, options, &mut random, &mut results)?;
                for (decoder, measured) in decoders.iter().zip(&all) {
                    let (name, label) = (&input.name, level.label());
                    print!("{name:<26} {label:>8} {:<14}", decoder.name);
                    summarize(measured, &mut random);
                }
            }
        }
    }
    eprintln!("wrote {}", path.display());
    Ok(())
}

/// The procedure over `input` at `level`: processes until every decoder's
/// intervals are narrow enough, or the budget is spent; returns what was
/// measured of each decoder of `codec` that offers checkpoints, in its
/// order, and records every place.
fn measure(
    codec: Codec,
    input: &Input,
    level: Level,
    options: &Options,
    random: &mut SplitMix,
    results: &mut fs::File,
) -> Result<Vec<Measured>> {
    let decoders = resumable(codec);
    let mut all: Vec<Measured> = decoders.iter().map(|_| Measured::default()).collect();
    let (bytes, packed) = (input.size()?, input.encoded(codec, level)?.len());
    for process in 0..procedure::MOST_PROCESSES {
        let narrow = |medians: &Vec<f64>| {
            medians.is_empty() || {
                let mut random = SplitMix(process as u64);
                let (median, low, high) = procedure::bootstrap(medians, &mut random);
                (high - low) / 2.0 <= median * procedure::WIDTH
            }
        };
        let done = |measured: &Measured| {
            measured.failure.is_some() || (narrow(&measured.take) && narrow(&measured.resume))
        };
        if process >= procedure::PROCESSES && all.iter().all(done) {
            break;
        }
        let output = procedure::harness(options)?
            .args(["checkpoints-sample", codec.name(), &input.name])
            .arg(level.label())
            .arg(random.next().to_string())
            .output()?;
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(format!("{}: {error}", input.name).into());
        }
        // Each decoder's places' median times, to take and to resume.
        let mut medians: Vec<(Vec<f64>, Vec<f64>)> = vec![Default::default(); decoders.len()];
        for line in String::from_utf8(output.stdout)?.lines() {
            let line: Line = serde_json::from_str(line)?;
            let (name, record) = match &line {
                Line::Checkpoint(taken) => (
                    &taken.decoder,
                    Record::Checkpoint {
                        codec: codec.name(),
                        input: &input.name,
                        level: level.label(),
                        piece: PIECE,
                        bytes,
                        packed,
                        process,
                        taken,
                    },
                ),
                Line::Failure { decoder, failure } => (
                    decoder,
                    Record::Failure {
                        codec: codec.name(),
                        input: &input.name,
                        level: level.label(),
                        piece: PIECE,
                        decoder,
                        failure,
                    },
                ),
            };
            writeln!(results, "{}", serde_json::to_string(&record)?)?;
            let i = decoders
                .iter()
                .position(|d| d.name == name)
                .ok_or("no such decoder")?;
            match line {
                Line::Checkpoint(taken) => {
                    medians[i].0.push(median(&taken.take, taken.takes));
                    medians[i].1.push(median(&taken.resume, taken.resumes));
                    all[i].held = all[i].held.max(taken.held);
                },
                Line::Failure { failure, .. } => all[i].failure = Some(failure),
            }
        }
        for (measured, (mut take, mut resume)) in all.iter_mut().zip(medians) {
            if !take.is_empty() {
                measured.take.push(procedure::median(&mut take));
                measured.resume.push(procedure::median(&mut resume));
            }
        }
    }
    Ok(all)
}

/// The median of `times`, in nanoseconds, each of `each`.
fn median(times: &[u64], each: usize) -> f64 {
    let mut times: Vec<f64> = times.iter().map(|&ns| ns as f64 / each as f64).collect();
    procedure::median(&mut times)
}

/// Prints the median over processes of `measured`'s times, each with its
/// interval, by resampling them with `random`, and the most heap one
/// held, if counted.
fn summarize(measured: &Measured, random: &mut SplitMix) {
    if let Some(failure) = &measured.failure {
        println!(" ({failure})");
        return;
    }
    if measured.take.is_empty() {
        println!(" (none taken)");
        return;
    }
    for medians in [&measured.take, &measured.resume] {
        let (median, low, high) = procedure::bootstrap(medians, random);
        let width = (high - low) / 2.0 / median * 100.0;
        print!(" {:>10.3} ±{width:>4.1}%", median / 1e3);
    }
    match measured.held {
        Some(held) => println!(" {:>10.1}", held as f64 / 1024.0),
        None => println!(" {:>10}", "-"),
    }
}

/// Decodes `packed` with `decoder` in pieces, measuring a checkpoint at
/// each place, and checks that it decoded `original`.
fn places(
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
