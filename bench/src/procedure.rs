// This is free and unencumbered software released into the public domain.

//! The procedure `METHOD.md` lays out. For each input and level, `run`
//! starts independent processes of the harness, `sample`, at least five,
//! each of which warms every decoder up and then times them in rounds, in
//! a random order each, checking every output, untimed. Of each decoder,
//! it takes the median iteration of each process, and across processes the
//! median of those medians, with a 95% bootstrap confidence interval; it
//! starts processes until every interval's half-width is within 1% of its
//! median, or its budget is spent. Each counterpart is compared with ours
//! by the ratio of their speeds in each process, paired, with an interval
//! likewise. Every sample goes to `bench/results/`, after what the run
//! records of the machine.

use crate::{
    Options, Result,
    corpus::{self, Input},
    decoders::{Codec, Decoder},
    inputs::{Level, SplitMix},
    machine::Machine,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write as _,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

/// How long each decoder warms up in a process, the least it is timed for,
/// and the fewest iterations.
const WARM_UP: Duration = Duration::from_secs(1);
const TIMED: Duration = Duration::from_millis(500);
const ITERATIONS: usize = 10;

/// The fewest processes per input and level, and the most.
const PROCESSES: usize = 5;
const MOST_PROCESSES: usize = 20;

/// The widest a confidence interval's half-width may be, as a fraction of
/// its median, before more processes are started.
const WIDTH: f64 = 0.01;

/// The resamples of each bootstrap.
const RESAMPLES: usize = 2000;

/// What a `sample` process prints, a line of JSON each: an iteration's
/// time, or why a decoder failed.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Line {
    Sample { decoder: String, ns: u64 },
    Failure { decoder: String, failure: String },
}

/// A line of the results' file: what the run records of the machine, then
/// each sample and failure, with what it was of.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Record<'a> {
    Machine(&'a Machine),
    Sample {
        codec: &'a str,
        input: &'a str,
        level: String,
        piece: Option<usize>,
        bytes: usize,
        process: usize,
        decoder: &'a str,
        ns: u64,
    },
    Failure {
        codec: &'a str,
        input: &'a str,
        level: String,
        decoder: &'a str,
        failure: &'a str,
    },
}

/// The `sample` command, in a process of its own: times every decoder of
/// `codec` over `input` encoded at `level`, given and taken in pieces of
/// `piece` bytes, in rounds in an order shuffled from `seed`, and prints
/// each iteration's time.
pub fn sample(codec: Codec, input: &str, level: Level, piece: usize, seed: u64) -> Result<()> {
    let input = corpus::inputs(input)?
        .into_iter()
        .find(|candidate| candidate.name == input)
        .ok_or("no such input")?;
    let (original, packed) = (input.bytes()?, input.encoded(codec, level)?);
    let mut output = vec![0; original.len()];
    let mut stdout = std::io::stdout().lock();
    let mut print = |line: &Line| -> Result<()> {
        writeln!(stdout, "{}", serde_json::to_string(line)?)?;
        Ok(())
    };
    // Times one decoding, and checks what it decoded, untimed.
    let mut run = |decoder: &Decoder| -> std::result::Result<Duration, String> {
        output.fill(0);
        let start = Instant::now();
        let len = (decoder.decode)(&packed, &mut output, piece)?;
        let elapsed = start.elapsed();
        match output[..len] == *original {
            true => Ok(elapsed),
            false => Err("wrong".into()),
        }
    };
    // Those that fail are left out from then on.
    let mut decoders: Vec<&Decoder> = Vec::new();
    for decoder in codec.decoders() {
        let start = Instant::now();
        let warmed = loop {
            match run(decoder) {
                Err(failure) => break Err(failure),
                Ok(_) if start.elapsed() >= WARM_UP => break Ok(()),
                Ok(_) => {},
            }
        };
        match warmed {
            Ok(()) => decoders.push(decoder),
            Err(failure) => print(&Line::Failure {
                decoder: decoder.name.into(),
                failure,
            })?,
        }
    }
    let (mut random, mut timed) = (SplitMix(seed), vec![Duration::ZERO; decoders.len()]);
    let mut order: Vec<usize> = (0..decoders.len()).collect();
    for round in 0.. {
        if round >= ITERATIONS && timed.iter().all(|&timed| timed >= TIMED) {
            break;
        }
        random.shuffle(&mut order);
        for &i in &order {
            let elapsed = run(decoders[i])?;
            timed[i] += elapsed;
            print(&Line::Sample {
                decoder: decoders[i].name.into(),
                ns: elapsed.as_nanos() as u64,
            })?;
        }
    }
    Ok(())
}

/// A decoder's figures over an input at a level.
struct Figures {
    /// The median iteration of each process, in nanoseconds; the fastest
    /// is in the raw data.
    medians: Vec<f64>,
    failure: Option<String>,
}

/// The median of `values`, which must not be empty.
fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    match values.len() % 2 {
        0 => (values[middle - 1] + values[middle]) / 2.0,
        _ => values[middle],
    }
}

/// The median of `values` and its 95% bootstrap confidence interval, by
/// resampling them with `random`.
fn bootstrap(values: &[f64], random: &mut SplitMix) -> (f64, f64, f64) {
    let mut resample = vec![0.0; values.len()];
    let mut medians: Vec<f64> = (0..RESAMPLES)
        .map(|_| {
            for value in &mut resample {
                *value = values[(random.next() % values.len() as u64) as usize];
            }
            median(&mut resample)
        })
        .collect();
    medians.sort_by(f64::total_cmp);
    let at = |fraction: f64| medians[((RESAMPLES - 1) as f64 * fraction).round() as usize];
    (median(&mut values.to_vec()), at(0.025), at(0.975))
}

/// The `run` command: the procedure over every input and level of each
/// codec, with a table of each decoder's MB/s, its interval, and its speed
/// relative to ours, printed as it goes, and every sample written to
/// `bench/results/`.
pub fn run(options: &Options, bench: &Path) -> Result<()> {
    let machine = Machine::now(bench);
    fs::create_dir_all(bench.join("results"))?;
    let path = bench.join("results").join(machine.file_name());
    let mut results = fs::File::create(&path)?;
    writeln!(
        results,
        "{}",
        serde_json::to_string(&Record::Machine(&machine))?
    )?;
    eprintln!("writing {}", path.display());
    let inputs = corpus::inputs(&options.only)?;
    let mut random = SplitMix(0x5EED);
    for &codec in &options.codecs {
        let decoders = codec.decoders();
        print!("{:<26} {:>8}", codec.name(), "level");
        for decoder in decoders {
            print!(" {:>22}", decoder.name);
        }
        println!();
        // Each counterpart's speed relative to ours, by level, to sum up.
        let mut ratios: Vec<(Level, usize, f64)> = Vec::new();
        for input in &inputs {
            for level in codec.levels(input.size()?) {
                let all = measure(codec, input, level, options, &mut random, &mut results)?;
                print!("{:<26} {:>8}", input.name, level.label());
                let bytes = input.size()? as f64;
                for (i, figures) in all.iter().enumerate() {
                    if let Some(failure) = &figures.failure {
                        print!(" {:>22}", format!("({failure})"));
                        continue;
                    }
                    let (median, low, high) = bootstrap(&figures.medians, &mut random);
                    let width = (high - low) / 2.0 / median * 100.0;
                    let rate = bytes / median * 1e3;
                    print!(" {:>13} ±{width:>4.1}%", format!("{rate:.1}"));
                    if let Some(ratio) = paired(&all[0], figures).filter(|_| i > 0) {
                        ratios.push((level, i, ratio));
                    }
                }
                println!();
            }
        }
        summarize(codec, decoders, &ratios);
    }
    eprintln!("wrote {}", path.display());
    Ok(())
}

/// The procedure over `input` at `level`: processes until every decoder's
/// interval is narrow enough, or the budget is spent; returns the figures
/// of each decoder of `codec`, in its order, and records every sample.
fn measure(
    codec: Codec,
    input: &Input,
    level: Level,
    options: &Options,
    random: &mut SplitMix,
    results: &mut fs::File,
) -> Result<Vec<Figures>> {
    let decoders = codec.decoders();
    let mut figures: Vec<Figures> = decoders
        .iter()
        .map(|_| Figures {
            medians: Vec::new(),
            failure: None,
        })
        .collect();
    let piece = (options.piece != usize::MAX).then_some(options.piece);
    let bytes = input.size()?;
    for process in 0..MOST_PROCESSES {
        let narrow = |figures: &Figures| {
            figures.failure.is_some() || {
                let mut random = SplitMix(process as u64);
                let (median, low, high) = bootstrap(&figures.medians, &mut random);
                (high - low) / 2.0 <= median * WIDTH
            }
        };
        if process >= PROCESSES && figures.iter().all(narrow) {
            break;
        }
        let mut command = match &options.cpu {
            Some(cpu) => {
                let mut command = Command::new("taskset");
                command.args(["-c", cpu]).arg(std::env::current_exe()?);
                command
            },
            None => Command::new(std::env::current_exe()?),
        };
        let output = command
            .args(["sample", codec.name(), &input.name, &level.label()])
            .arg(options.piece.to_string())
            .arg(random.next().to_string())
            .output()?;
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(format!("{}: {error}", input.name).into());
        }
        let mut times: Vec<Vec<f64>> = vec![Vec::new(); decoders.len()];
        for line in String::from_utf8(output.stdout)?.lines() {
            let line: Line = serde_json::from_str(line)?;
            let (name, record) = match &line {
                Line::Sample { decoder, ns } => (
                    decoder,
                    Record::Sample {
                        codec: codec.name(),
                        input: &input.name,
                        level: level.label(),
                        piece,
                        bytes,
                        process,
                        decoder,
                        ns: *ns,
                    },
                ),
                Line::Failure { decoder, failure } => (
                    decoder,
                    Record::Failure {
                        codec: codec.name(),
                        input: &input.name,
                        level: level.label(),
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
                Line::Sample { ns, .. } => times[i].push(ns as f64),
                Line::Failure { failure, .. } => figures[i].failure = Some(failure),
            }
        }
        for (figures, mut times) in figures.iter_mut().zip(times) {
            if !times.is_empty() {
                figures.medians.push(median(&mut times));
            }
        }
    }
    Ok(figures)
}

/// The speed of `theirs` relative to `ours`, paired by process: the median
/// of their ratios, if both decoded in every process.
fn paired(ours: &Figures, theirs: &Figures) -> Option<f64> {
    if ours.medians.len() != theirs.medians.len() || ours.medians.is_empty() {
        return None;
    }
    let mut ratios: Vec<f64> = ours
        .medians
        .iter()
        .zip(&theirs.medians)
        .map(|(o, t)| o / t)
        .collect();
    Some(median(&mut ratios))
}

/// Prints each counterpart's speed relative to ours, per level, as the
/// geometric mean of the ratios over the inputs.
fn summarize(codec: Codec, decoders: &[Decoder], ratios: &[(Level, usize, f64)]) {
    println!(
        "{} relative to ours: the geometric mean over inputs",
        codec.name()
    );
    let mut levels: Vec<Level> = ratios.iter().map(|&(level, ..)| level).collect();
    levels.sort_by_key(|level| (level.number, level.long));
    levels.dedup();
    for level in levels {
        print!("{:<26} {:>8}", "", level.label());
        for (i, decoder) in decoders.iter().enumerate().skip(1) {
            let logs: Vec<f64> = ratios
                .iter()
                .filter(|&&(l, d, _)| l == level && d == i)
                .map(|&(.., ratio)| ratio.ln())
                .collect();
            let mean = (logs.iter().sum::<f64>() / logs.len() as f64).exp();
            print!(" {:>13} {mean:>7.2}x", decoder.name);
        }
        println!();
    }
}
