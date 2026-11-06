// This is free and unencumbered software released into the public domain.

//! The procedure of `procedure`, for the checksums and hashes: for each
//! algorithm, buffer size, and way of feeding it, `hash` starts processes
//! of the harness, `hash-sample`, at least five, each of which checks every
//! implementation against ours, warms each up, and times them in rounds, in
//! a random order each. A round times a batch of computations of at least
//! 64 KiB in all, as one of 16 bytes takes less than a timer resolves, and
//! of 5 ms or more, so that fast ones take no more rounds, and samples,
//! than slow ones. The statistics are the decoders'. It prints a Markdown table for each
//! algorithm, for its crate's `BENCH.md`, and writes every sample to
//! `bench/results/`.

use crate::{
    Options, Result,
    hashes::{Algorithm, Implementation},
    inputs::SplitMix,
    machine::Machine,
    procedure::{self, Figures},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    hint::black_box,
    io::Write as _,
    path::Path,
    time::{Duration, Instant},
};

/// The sizes of buffers, and the pieces fed: whole, or 7 bytes at a time.
pub const SIZES: [usize; 5] = [16, 256, 4 << 10, 64 << 10, 1 << 20];
pub const PIECES: [usize; 2] = [usize::MAX, 7];

/// How long each implementation warms up in a process, the least it is
/// timed for, and the fewest rounds.
const WARM_UP: Duration = Duration::from_millis(200);
const TIMED: Duration = Duration::from_millis(250);
const ROUNDS: usize = 10;

/// The fewest bytes a round computes over, and the least time it takes.
const BATCH: usize = 64 << 10;
const ROUND: Duration = Duration::from_millis(5);

/// What a `hash-sample` process prints, a line of JSON each: a round's
/// time and its computations, or why an implementation failed.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Line {
    Sample {
        implementation: String,
        iterations: usize,
        ns: u64,
    },
    Failure {
        implementation: String,
        failure: String,
    },
}

/// A line of the results' file, after the machine's.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Record<'a> {
    Machine(&'a Machine),
    Hash {
        algorithm: &'a str,
        bytes: usize,
        piece: Option<usize>,
        process: usize,
        implementation: &'a str,
        iterations: usize,
        ns: u64,
    },
    Failure {
        algorithm: &'a str,
        bytes: usize,
        piece: Option<usize>,
        implementation: &'a str,
        failure: &'a str,
    },
}

/// The bytes computed over: random, the same in every process.
fn buffer(size: usize) -> Vec<u8> {
    let mut random = SplitMix(0xF11E_5EED);
    (0..size).map(|_| random.next() as u8).collect()
}

/// The `hash-sample` command, in a process of its own: times every
/// implementation of `algorithm` over `size` bytes, fed in pieces of
/// `piece`, in rounds in an order shuffled from `seed`, and prints each
/// round's time.
pub fn sample(algorithm: Algorithm, size: usize, piece: usize, seed: u64) -> Result<()> {
    let bytes = buffer(size);
    let mut stdout = std::io::stdout().lock();
    let mut print = |line: &Line| -> Result<()> {
        writeln!(stdout, "{}", serde_json::to_string(line)?)?;
        Ok(())
    };
    let run = |implementation: &Implementation, iterations: usize| {
        let start = Instant::now();
        for _ in 0..iterations {
            black_box((implementation.compute)(black_box(&bytes), piece));
        }
        start.elapsed()
    };
    // Those that compute otherwise than ours, whole, are left out.
    let all = algorithm.implementations();
    let expected = (all[0].compute)(&bytes, usize::MAX);
    // Each with the computations a round of it takes.
    let mut implementations: Vec<(&Implementation, usize)> = Vec::new();
    for implementation in all {
        if (implementation.compute)(&bytes, piece) != expected {
            print(&Line::Failure {
                implementation: implementation.name.into(),
                failure: "wrong".into(),
            })?;
            continue;
        }
        let mut iterations = BATCH.div_ceil(size);
        let start = Instant::now();
        while start.elapsed() < WARM_UP {
            run(implementation, iterations);
        }
        let once = run(implementation, iterations).max(Duration::from_nanos(1));
        if once < ROUND {
            let more = ROUND.as_nanos().div_ceil(once.as_nanos());
            iterations = iterations.saturating_mul(usize::try_from(more)?);
        }
        implementations.push((implementation, iterations));
    }
    let mut random = SplitMix(seed);
    let mut timed = vec![Duration::ZERO; implementations.len()];
    let mut order: Vec<usize> = (0..implementations.len()).collect();
    for round in 0.. {
        if round >= ROUNDS && timed.iter().all(|&timed| timed >= TIMED) {
            break;
        }
        random.shuffle(&mut order);
        for &i in &order {
            let (implementation, iterations) = implementations[i];
            let elapsed = run(implementation, iterations);
            timed[i] += elapsed;
            print(&Line::Sample {
                implementation: implementation.name.into(),
                iterations,
                ns: elapsed.as_nanos() as u64,
            })?;
        }
    }
    Ok(())
}

/// The `hash` command: the procedure over every size and way of feeding
/// of each algorithm, with a Markdown table of each implementation's GB/s
/// (10^9 bytes a second), its interval, and each counterpart's speed
/// relative to ours, and every sample written to `bench/results/`.
pub fn run(options: &Options, algorithms: &[Algorithm], bench: &Path) -> Result<()> {
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
    let build = match cfg!(feature = "intrinsics") {
        true => "with `intrinsics`",
        false => "portable",
    };
    println!("On {}, {}; ours {build}.", machine.cpu, machine.os);
    let mut random = SplitMix(0x5EED);
    for &algorithm in algorithms {
        let implementations = algorithm.implementations();
        println!();
        println!("### {} (`{}`)", algorithm.name(), algorithm.krate());
        println!();
        let named: Vec<String> = implementations
            .iter()
            .map(|i| format!("{}, `{}`, in {}", i.name, i.krate, i.language))
            .collect();
        println!("{}.", named.join("; "));
        println!();
        print!("| Bytes | Fed |");
        for implementation in implementations {
            print!(" {} |", implementation.name);
        }
        println!();
        print!("| ---: | --- |");
        for _ in implementations {
            print!(" ---: |");
        }
        println!();
        // Each counterpart's speed relative to ours, by way of feeding.
        let mut ratios: Vec<(usize, usize, f64)> = Vec::new();
        for piece in PIECES {
            for size in SIZES {
                let all = measure(algorithm, size, piece, options, &mut random, &mut results)?;
                let fed = match piece {
                    usize::MAX => "whole".to_owned(),
                    piece => format!("by {piece}"),
                };
                print!("| {size} | {fed} |");
                for (i, figures) in all.iter().enumerate() {
                    if let Some(failure) = &figures.failure {
                        print!(" ({failure}) |");
                        continue;
                    }
                    let (median, low, high) = procedure::bootstrap(&figures.medians, &mut random);
                    let width = (high - low) / 2.0 / median * 100.0;
                    print!(" {:.2} ± {width:.1}% |", size as f64 / median);
                    if let Some(ratio) = procedure::paired(&all[0], figures).filter(|_| i > 0) {
                        ratios.push((piece, i, ratio));
                    }
                }
                println!();
            }
        }
        if implementations.len() > 1 {
            summarize(implementations, &ratios);
        }
    }
    eprintln!("wrote {}", path.display());
    Ok(())
}

/// The procedure over `size` bytes of `algorithm` fed in pieces of
/// `piece`: processes until every implementation's interval is narrow
/// enough, or the budget is spent; returns the figures of each, in its
/// order, as nanoseconds per computation, and records every sample.
fn measure(
    algorithm: Algorithm,
    size: usize,
    piece: usize,
    options: &Options,
    random: &mut SplitMix,
    results: &mut fs::File,
) -> Result<Vec<Figures>> {
    let implementations = algorithm.implementations();
    let mut figures: Vec<Figures> = implementations
        .iter()
        .map(|_| Figures {
            medians: Vec::new(),
            failure: None,
        })
        .collect();
    let recorded = (piece != usize::MAX).then_some(piece);
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
            .args(["hash-sample", algorithm.name()])
            .args([size.to_string(), piece.to_string()])
            .arg(random.next().to_string())
            .output()?;
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(format!("{}: {error}", algorithm.name()).into());
        }
        let mut times: Vec<Vec<f64>> = vec![Vec::new(); implementations.len()];
        for line in String::from_utf8(output.stdout)?.lines() {
            let line: Line = serde_json::from_str(line)?;
            let (name, record) = match &line {
                Line::Sample {
                    implementation,
                    iterations,
                    ns,
                } => (
                    implementation,
                    Record::Hash {
                        algorithm: algorithm.name(),
                        bytes: size,
                        piece: recorded,
                        process,
                        implementation,
                        iterations: *iterations,
                        ns: *ns,
                    },
                ),
                Line::Failure {
                    implementation,
                    failure,
                } => (
                    implementation,
                    Record::Failure {
                        algorithm: algorithm.name(),
                        bytes: size,
                        piece: recorded,
                        implementation,
                        failure,
                    },
                ),
            };
            writeln!(results, "{}", serde_json::to_string(&record)?)?;
            let i = implementations
                .iter()
                .position(|candidate| candidate.name == name)
                .ok_or("no such implementation")?;
            match line {
                Line::Sample { iterations, ns, .. } => times[i].push(ns as f64 / iterations as f64),
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

/// Prints each counterpart's speed relative to ours, whole and in pieces,
/// as the geometric mean of the ratios over the sizes.
fn summarize(implementations: &[Implementation], ratios: &[(usize, usize, f64)]) {
    println!();
    print!("Relative to ours, the geometric mean over sizes:");
    for piece in PIECES {
        let fed = match piece {
            usize::MAX => "whole".to_owned(),
            piece => format!("by {piece}"),
        };
        print!(" {fed},");
        for (i, implementation) in implementations.iter().enumerate().skip(1) {
            let logs: Vec<f64> = ratios
                .iter()
                .filter(|&&(p, d, _)| p == piece && d == i)
                .map(|&(.., ratio)| ratio.ln())
                .collect();
            if logs.is_empty() {
                continue;
            }
            let mean = (logs.iter().sum::<f64>() / logs.len() as f64).exp();
            print!(" {} {mean:.2}x", implementation.name);
        }
        print!(";");
    }
    println!();
}
