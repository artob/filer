// This is free and unencumbered software released into the public domain.

//! Small streams, as `METHOD.md` asks: the time per stream of packed
//! streams of about 64 B, 1 KiB, and 16 KiB, each decoded by a decoder
//! made for it, its making timed with it, as archives of many small
//! members are read.
//!
//! A size's set of streams is cut from the corpus's inputs, not the
//! synthetic ones, at `PLACES` places spread over each: at each, the
//! shortest slice that its codec's reference encoder, at its default
//! level, packs into that size or more. The same slices are encoded at
//! each of the codec's levels, and each set is written whole, the slices
//! with their streams, as `small/<size>`, recorded in `packed.toml` as the
//! inputs' encodings are, so that every machine decodes the same streams.
//!
//! `small` measures each set by the procedure of `procedure`, in rounds as
//! `throughput`'s: a `small-sample` process times rounds of passes over
//! the set, each round 5 ms or more, each pass decoding every stream in
//! turn into its own place, and checks what the last pass decoded,
//! untimed. Its figures are the time per stream.

use crate::{
    Options, Result, corpus,
    decoders::{Codec, Decoder, Failure},
    inputs::{Level, SplitMix},
    machine::Machine,
    procedure::{self, Figures},
    throughput,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write as _,
    ops::Range,
    path::Path,
    time::{Duration, Instant},
};

/// The sizes of the streams of each set, packed, about.
pub const SIZES: [usize; 3] = [64, 1 << 10, 16 << 10];

/// The places in each input that a stream is cut from.
const PLACES: usize = 8;

/// The name of the set of streams of about `size` bytes.
fn name(size: usize) -> String {
    format!("small/{size}")
}

/// What a `small-sample` process prints, a line of JSON each: a round's
/// time and its passes, or why a decoder failed.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Line {
    Sample {
        decoder: String,
        passes: usize,
        ns: u64,
    },
    Failure {
        decoder: String,
        failure: String,
    },
}

/// A line of the results' file, after the machine's.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Record<'a> {
    Machine(&'a Machine),
    Small {
        codec: &'a str,
        level: String,
        /// The streams' size, about, and how many there are; and their
        /// bytes in all, and packed.
        size: usize,
        streams: usize,
        bytes: usize,
        packed: usize,
        process: usize,
        decoder: &'a str,
        passes: usize,
        ns: u64,
    },
    Failure {
        codec: &'a str,
        level: String,
        size: usize,
        decoder: &'a str,
        failure: &'a str,
    },
}

/// A set of streams: the slices they decode to, one after another, and
/// the streams, one after another, with where each is among them and the
/// length of its slice.
struct Set {
    original: Vec<u8>,
    packed: Vec<u8>,
    streams: Vec<(Range<usize>, usize)>,
}

impl Set {
    /// Reads a set as `write` writes it.
    fn read(mut bytes: &[u8]) -> Result<Self> {
        let mut set = Self {
            original: Vec::new(),
            packed: Vec::new(),
            streams: Vec::new(),
        };
        while !bytes.is_empty() {
            let original = piece(&mut bytes)?;
            let packed = piece(&mut bytes)?;
            let at = set.packed.len();
            set.original.extend_from_slice(original);
            set.packed.extend_from_slice(packed);
            set.streams.push((at..set.packed.len(), original.len()));
        }
        match set.streams.is_empty() {
            true => Err("a set of no streams".into()),
            false => Ok(set),
        }
    }
}

/// Takes from `bytes` the piece at its start, after its length.
fn piece<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8]> {
    let (len, rest) = bytes.split_first_chunk::<4>().ok_or("a set cut short")?;
    let len = u32::from_le_bytes(*len) as usize;
    let piece = rest.get(..len).ok_or("a set cut short")?;
    *bytes = &rest[len..];
    Ok(piece)
}

/// Writes each of `slices` and its stream as `codec` encodes it at
/// `level`, each after its length, four bytes, little-endian.
fn write(codec: Codec, level: Level, slices: &[Vec<u8>]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for slice in slices {
        let packed = codec.encode(slice, level)?;
        for part in [slice, &packed] {
            bytes.extend_from_slice(&u32::try_from(part.len())?.to_le_bytes());
            bytes.extend_from_slice(part);
        }
    }
    Ok(bytes)
}

/// The slices of `data` that `codec` packs, at its default level, into
/// `size` bytes or more, and without their last byte into less: one at
/// each of `PLACES` places spread over it, or after the slice before,
/// while what is left packs into that much.
fn cut(codec: Codec, size: usize, data: &[u8]) -> Result<Vec<Vec<u8>>> {
    let level = codec.default_level();
    let (mut slices, mut end) = (Vec::new(), 0);
    for place in 0..PLACES {
        let start = (data.len() * place / PLACES).max(end);
        let rest = &data[start..];
        let packs =
            |len: usize| -> Result<bool> { Ok(codec.encode(&rest[..len], level)?.len() >= size) };
        // Doubled until it packs into enough, then halved back to the
        // shortest that does.
        let (mut short, mut long) = (0, size.min(rest.len()));
        while !packs(long)? {
            if long == rest.len() {
                return Ok(slices);
            }
            (short, long) = (long, (long * 2).min(rest.len()));
        }
        while long - short > 1 {
            let middle = short + (long - short) / 2;
            match packs(middle)? {
                true => long = middle,
                false => short = middle,
            }
        }
        slices.push(rest[..long].to_vec());
        end = start + long;
    }
    Ok(slices)
}

/// Cuts and encodes each of `codec`'s sets whose name begins with
/// `prefix`, unless it is here at every level as `packed.toml` records it;
/// checks each against `packed.toml`, and records it there if new.
pub fn encode(codec: Codec, prefix: &str) -> Result<()> {
    let levels = codec.levels(0);
    let sizes: Vec<usize> = SIZES
        .into_iter()
        .filter(|&size| name(size).starts_with(prefix))
        .filter(|&size| {
            let here = |&level| corpus::set(codec, level, &name(size)).is_ok();
            !levels.iter().all(here)
        })
        .collect();
    if sizes.is_empty() {
        return Ok(());
    }
    let mut slices = vec![Vec::new(); sizes.len()];
    for input in corpus::inputs("")? {
        if input.synthetic() {
            continue;
        }
        eprintln!("cutting {} for {}", input.name, codec.name());
        let data = input.bytes()?;
        for (slices, &size) in slices.iter_mut().zip(&sizes) {
            slices.extend(cut(codec, size, &data)?);
        }
    }
    for (slices, &size) in slices.iter().zip(&sizes) {
        for &level in &levels {
            eprintln!("encoding {} with {} at {level}", name(size), codec.name());
            let bytes = write(codec, level, slices)?;
            corpus::record_set(codec, level, &name(size), &bytes)?;
        }
    }
    Ok(())
}

/// The `small-sample` command, in a process of its own: times every
/// decoder of `codec` over the set of streams of about `size` bytes that
/// it encoded at `level`, in rounds in an order shuffled from `seed`, and
/// prints each round's time.
pub fn sample(codec: Codec, level: Level, size: usize, seed: u64) -> Result<()> {
    let set = Set::read(&corpus::set(codec, level, &name(size))?)?;
    let mut output = vec![0; set.original.len()];
    let mut stdout = std::io::stdout().lock();
    let mut print = |line: &Line| -> Result<()> {
        writeln!(stdout, "{}", serde_json::to_string(line)?)?;
        Ok(())
    };
    // Times `passes` passes, each decoding every stream into its place, and
    // checks what the last decoded, untimed.
    let mut run = |decoder: &Decoder, passes: usize| -> std::result::Result<Duration, Failure> {
        output.fill(0);
        let start = Instant::now();
        for _ in 0..passes {
            let mut at = 0;
            for (packed, len) in &set.streams {
                let into = &mut output[at..at + len];
                if (decoder.decode)(&set.packed[packed.clone()], into, usize::MAX)? != *len {
                    return Err("wrong".into());
                }
                at += len;
            }
        }
        let elapsed = start.elapsed();
        match output == set.original {
            true => Ok(elapsed),
            false => Err("wrong".into()),
        }
    };
    // Those that fail are left out from then on; each with the passes a
    // round of it takes.
    let mut decoders: Vec<(&Decoder, usize)> = Vec::new();
    for decoder in codec.fed(usize::MAX) {
        let start = Instant::now();
        let warmed = loop {
            match run(decoder, 1) {
                Err(failure) => break Err(failure),
                Ok(_) if start.elapsed() >= procedure::WARM_UP => break run(decoder, 1),
                Ok(_) => {},
            }
        };
        match warmed {
            Ok(once) => {
                let once = once.max(Duration::from_nanos(1)).as_nanos();
                let passes = throughput::ROUND.as_nanos().div_ceil(once).max(1);
                decoders.push((decoder, usize::try_from(passes)?));
            },
            Err(failure) => print(&Line::Failure {
                decoder: decoder.name.into(),
                failure,
            })?,
        }
    }
    let (mut random, mut timed) = (SplitMix(seed), vec![Duration::ZERO; decoders.len()]);
    let mut order: Vec<usize> = (0..decoders.len()).collect();
    for round in 0.. {
        let enough = timed.iter().all(|&timed| timed >= procedure::TIMED);
        if round >= procedure::ITERATIONS && enough {
            break;
        }
        random.shuffle(&mut order);
        for &i in &order {
            let (decoder, passes) = decoders[i];
            let elapsed = run(decoder, passes)?;
            timed[i] += elapsed;
            print(&Line::Sample {
                decoder: decoder.name.into(),
                passes,
                ns: elapsed.as_nanos() as u64,
            })?;
        }
    }
    Ok(())
}

/// The `small` command: the procedure over each set of each codec, at
/// each of its levels, with a table of each decoder's microseconds per
/// stream, its interval, and its speed relative to ours, printed as it
/// goes, and every sample written to `bench/results/`.
pub fn run(options: &Options, bench: &Path) -> Result<()> {
    let machine = Machine::now(bench);
    let (path, mut results) = machine.create(bench)?;
    writeln!(
        results,
        "{}",
        serde_json::to_string(&Record::Machine(&machine))?
    )?;
    eprintln!("writing {}", path.display());
    let mut random = SplitMix(0x5EED);
    for &codec in &options.codecs {
        let decoders = codec.fed(usize::MAX);
        println!("{}, µs per stream:", codec.name());
        print!("{:<14} {:>8}", "streams", "level");
        for decoder in &decoders {
            print!(" {:>22}", decoder.name);
        }
        println!();
        // Each counterpart's speed relative to ours, by size, to sum up.
        let mut ratios: Vec<(usize, usize, f64)> = Vec::new();
        for size in SIZES {
            for level in codec.levels(0) {
                let all = measure(codec, level, size, options, &mut random, &mut results)?;
                print!("{:<14} {:>8}", name(size), level.label());
                for (i, figures) in all.iter().enumerate() {
                    if let Some(failure) = &figures.failure {
                        print!(" {:>22}", format!("({failure})"));
                        continue;
                    }
                    let (median, low, high) = procedure::bootstrap(&figures.medians, &mut random);
                    let width = (high - low) / 2.0 / median * 100.0;
                    print!(" {:>13} ±{width:>4.1}%", format!("{:.3}", median / 1e3));
                    if let Some(ratio) = procedure::paired(&all[0], figures).filter(|_| i > 0) {
                        ratios.push((size, i, ratio));
                    }
                }
                println!();
            }
        }
        summarize(codec, &decoders, &ratios);
    }
    eprintln!("wrote {}", path.display());
    Ok(())
}

/// The procedure over the set of streams of about `size` bytes that
/// `codec` encoded at `level`: processes until every decoder's interval
/// is narrow enough, or the budget is spent; returns the figures of each
/// decoder of `codec`, in its order, as nanoseconds per stream, and
/// records every sample.
fn measure(
    codec: Codec,
    level: Level,
    size: usize,
    options: &Options,
    random: &mut SplitMix,
    results: &mut fs::File,
) -> Result<Vec<Figures>> {
    let decoders = codec.fed(usize::MAX);
    let mut figures: Vec<Figures> = decoders
        .iter()
        .map(|_| Figures {
            medians: Vec::new(),
            failure: None,
        })
        .collect();
    let set = Set::read(&corpus::set(codec, level, &name(size))?)?;
    let (streams, bytes, packed) = (set.streams.len(), set.original.len(), set.packed.len());
    for process in 0..procedure::MOST_PROCESSES {
        let narrow = |figures: &Figures| {
            figures.failure.is_some() || {
                let mut random = SplitMix(process as u64);
                let (median, low, high) = procedure::bootstrap(&figures.medians, &mut random);
                (high - low) / 2.0 <= median * procedure::WIDTH
            }
        };
        if process >= procedure::PROCESSES && figures.iter().all(narrow) {
            break;
        }
        let output = procedure::harness(options)?
            .args(["small-sample", codec.name(), &level.label()])
            .arg(size.to_string())
            .arg(random.next().to_string())
            .output()?;
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(format!("{} at {level}: {error}", name(size)).into());
        }
        let mut times: Vec<Vec<f64>> = vec![Vec::new(); decoders.len()];
        for line in String::from_utf8(output.stdout)?.lines() {
            let line: Line = serde_json::from_str(line)?;
            let (which, record) = match &line {
                Line::Sample {
                    decoder,
                    passes,
                    ns,
                } => (
                    decoder,
                    Record::Small {
                        codec: codec.name(),
                        level: level.label(),
                        size,
                        streams,
                        bytes,
                        packed,
                        process,
                        decoder,
                        passes: *passes,
                        ns: *ns,
                    },
                ),
                Line::Failure { decoder, failure } => (
                    decoder,
                    Record::Failure {
                        codec: codec.name(),
                        level: level.label(),
                        size,
                        decoder,
                        failure,
                    },
                ),
            };
            writeln!(results, "{}", serde_json::to_string(&record)?)?;
            let i = decoders
                .iter()
                .position(|d| d.name == which)
                .ok_or("no such decoder")?;
            match line {
                Line::Sample { passes, ns, .. } => {
                    times[i].push(ns as f64 / (passes * streams) as f64);
                },
                Line::Failure { failure, .. } => figures[i].failure = Some(failure),
            }
        }
        for (figures, mut times) in figures.iter_mut().zip(times) {
            if !times.is_empty() {
                figures.medians.push(procedure::median(&mut times));
            }
        }
    }
    Ok(figures)
}

/// Prints each counterpart's speed relative to ours, per size, as the
/// geometric mean of the ratios over the levels.
fn summarize(codec: Codec, decoders: &[&Decoder], ratios: &[(usize, usize, f64)]) {
    println!(
        "{} relative to ours: the geometric mean over levels",
        codec.name()
    );
    for size in SIZES {
        print!("{:<14} {:>8}", name(size), "");
        for (i, decoder) in decoders.iter().enumerate().skip(1) {
            let logs: Vec<f64> = ratios
                .iter()
                .filter(|&&(s, d, _)| s == size && d == i)
                .map(|&(.., ratio)| ratio.ln())
                .collect();
            if logs.is_empty() {
                continue;
            }
            let mean = (logs.iter().sum::<f64>() / logs.len() as f64).exp();
            print!(" {:>13} {mean:>7.2}x", decoder.name);
        }
        println!();
    }
}
