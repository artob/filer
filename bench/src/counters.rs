// This is free and unencumbered software released into the public domain.

//! The hardware counters, as `METHOD.md` asks: on Linux, `counters` counts
//! each decoder's cycles, instructions, branch misses, and L1 data cache
//! misses over every input at every level, by `perf stat`, in user space,
//! to explain differences in speed, never to rank.
//!
//! Each decoder is counted in a `counters-sample` process of its own under
//! `perf stat`, a pair of events at a time: a virtual machine's counters
//! are few, and more events than counters are multiplexed, each counted
//! part of the time and estimated (Neoverse V3's under EC2 count three at
//! once). The process decodes for `WARM_UP`, its counters off, then turns
//! them on, through `perf stat`'s control FIFO, for as many passes as take
//! `COUNTED`, or one, turns them off, and checks what it decoded. Its
//! figures are the counts over those passes, recorded with how many there
//! were, as the counts are, give or take a cycle's noise, deterministic:
//! one process each suffices, as for `memory`.
//!
//! On AMD's cores, L1 data cache misses are counted as the data cache's
//! requests to L2, as EC2's hypervisor hides the load-store unit's events,
//! by which `perf`'s generic `L1-dcache-load-misses` counts them there.

use crate::{
    Options, Result,
    corpus::{self, Input},
    decoders::{Codec, Decoder},
    inputs::Level,
    machine::Machine,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufRead as _, BufReader, Write as _},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

/// How long each decoder warms up in a process, its counters off, and
/// about how long its counted passes take.
const WARM_UP: Duration = Duration::from_millis(100);
const COUNTED: Duration = Duration::from_millis(200);

/// What a `counters-sample` process prints, a line of JSON: how many passes
/// it counted, or why its decoder failed.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Line {
    Counted { passes: usize },
    Failure { failure: String },
}

/// A line of the results' file, after the machine's.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Record<'a> {
    Machine(&'a Machine),
    Counter {
        codec: &'a str,
        input: &'a str,
        level: String,
        piece: Option<usize>,
        /// The input's size, and packed.
        bytes: usize,
        packed: usize,
        decoder: &'a str,
        /// The event, as `perf` names it, and its count over `passes`
        /// passes, if counted; and the share of the time it was counted,
        /// in percent, which is less if it was multiplexed.
        event: &'a str,
        passes: usize,
        count: Option<u64>,
        running: f64,
    },
    Failure {
        codec: &'a str,
        input: &'a str,
        level: String,
        piece: Option<usize>,
        decoder: &'a str,
        failure: &'a str,
    },
}

/// A directory of the run's own, removed with what it holds when dropped,
/// however the run ends.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

/// The events counted, in pairs, each pair in a process of its own: on
/// AMD's cores, the data cache's requests to L2 for its misses.
fn groups(machine: &Machine) -> [[&'static str; 2]; 2] {
    let misses = match machine.cpu.contains("AMD") {
        true => "l2_cache_req_stat.dc_access_in_l2",
        false => "L1-dcache-load-misses",
    };
    [["cycles", "instructions"], ["branch-misses", misses]]
}

/// The `counters-sample` command, in a process of its own under `perf
/// stat`, its counters off: warms `decoder` of `codec` up over `input`
/// encoded at `level`, given and taken in pieces of `piece` bytes; then
/// turns the counters on through the FIFOs in `fifos`, decodes, turns them
/// off, and prints how many passes it counted.
pub fn sample(
    codec: Codec,
    input: &str,
    level: Level,
    piece: usize,
    decoder: &str,
    fifos: &Path,
) -> Result<()> {
    let input = corpus::inputs(input)?
        .into_iter()
        .find(|candidate| candidate.name == input)
        .ok_or("no such input")?;
    let decoder = codec
        .fed(piece)
        .into_iter()
        .find(|candidate| candidate.name == decoder)
        .ok_or("no such decoder")?;
    let (original, packed) = (input.bytes()?, input.encoded(codec, level)?);
    let mut output = vec![0; original.len()];
    let print = |line: &Line| -> Result<()> {
        println!("{}", serde_json::to_string(line)?);
        Ok(())
    };
    // Decodes `passes` times, and checks what the last decoded.
    let mut run = |passes: usize| -> std::result::Result<(), String> {
        output.fill(0);
        let mut len = 0;
        for _ in 0..passes {
            len = (decoder.decode)(&packed, &mut output, piece)?;
        }
        match output[..len] == *original {
            true => Ok(()),
            false => Err("wrong".into()),
        }
    };
    let (start, mut warmed) = (Instant::now(), 0);
    while warmed == 0 || start.elapsed() < WARM_UP {
        if let Err(failure) = run(1) {
            return print(&Line::Failure { failure });
        }
        warmed += 1;
    }
    let once = start.elapsed() / warmed;
    let passes = COUNTED.as_nanos().div_ceil(once.as_nanos().max(1)).max(1);
    let passes = usize::try_from(passes)?;
    let mut control = fs::OpenOptions::new()
        .write(true)
        .open(fifos.join("control"))?;
    let mut ack = BufReader::new(fs::File::open(fifos.join("ack"))?);
    let mut say = |command: &str| -> Result<()> {
        writeln!(control, "{command}")?;
        ack.read_line(&mut String::new())?;
        Ok(())
    };
    say("enable")?;
    let decoded = run(passes);
    say("disable")?;
    match decoded {
        Ok(()) => print(&Line::Counted { passes }),
        Err(failure) => print(&Line::Failure { failure }),
    }
}

/// The `counters` command: counts every decoder of each codec over every
/// input at every level, given and taken in pieces of `options.piece`, by
/// `perf stat`; prints a table of each count per byte decoded and the
/// instructions per cycle, and writes each to `bench/results/`.
pub fn run(options: &Options, bench: &Path) -> Result<()> {
    let machine = Machine::now(bench);
    let (path, mut results) = machine.create(bench)?;
    let mut record = |record: &Record| -> Result<()> {
        writeln!(results, "{}", serde_json::to_string(record)?)?;
        Ok(())
    };
    record(&Record::Machine(&machine))?;
    eprintln!("writing {}", path.display());
    // The FIFOs through which each process turns its counters on, and
    // `perf stat` answers, in a directory of the run's own.
    let scratch = Scratch(std::env::temp_dir().join(format!("filer-bench-{}", std::process::id())));
    let fifos = &scratch.0;
    fs::create_dir_all(fifos)?;
    for fifo in ["control", "ack"] {
        if !Command::new("mkfifo")
            .arg(fifos.join(fifo))
            .status()?
            .success()
        {
            return Err("mkfifo failed".into());
        }
    }
    let groups = groups(&machine);
    let inputs = corpus::inputs(&options.only)?;
    let piece = (options.piece != usize::MAX).then_some(options.piece);
    for &codec in &options.codecs {
        println!(
            "{:<26} {:>8} {:<14} {:>10} {:>10} {:>6} {:>12} {:>12}",
            codec.name(),
            "level",
            "decoder",
            "cycles/B",
            "instr./B",
            "IPC",
            "br. miss/B",
            "L1d miss/B"
        );
        for input in &inputs {
            let bytes = input.size()?;
            for level in codec.levels(bytes) {
                let packed = input.encoded(codec, level)?.len();
                for decoder in codec.fed(options.piece) {
                    let (codec, name, label) = (codec.name(), &input.name, level.label());
                    print!("{name:<26} {label:>8} {:<14}", decoder.name);
                    let mut counts: Vec<Option<f64>> = Vec::new();
                    for events in &groups {
                        let counted = count(options, codec, input, level, decoder, events, fifos)?;
                        let (passes, counted) = match counted {
                            Ok(counted) => counted,
                            Err(failure) => {
                                print!(" ({failure})");
                                record(&Record::Failure {
                                    codec,
                                    input: name,
                                    level: label.clone(),
                                    piece,
                                    decoder: decoder.name,
                                    failure: &failure,
                                })?;
                                break;
                            },
                        };
                        for (event, (count, running)) in events.iter().zip(counted) {
                            let decoded = (passes * bytes) as f64;
                            counts.push(count.map(|count| count as f64 / decoded));
                            record(&Record::Counter {
                                codec,
                                input: name,
                                level: label.clone(),
                                piece,
                                bytes,
                                packed,
                                decoder: decoder.name,
                                event,
                                passes,
                                count,
                                running,
                            })?;
                        }
                    }
                    if let [cycles, instructions, branches, misses] = counts[..] {
                        let shown = |count: Option<f64>, width: usize, places: usize| match count {
                            Some(count) => format!(" {count:>width$.places$}"),
                            None => format!(" {:>width$}", "-"),
                        };
                        let ipc = instructions.zip(cycles).map(|(i, c)| i / c);
                        print!(
                            "{}{}{}{}{}",
                            shown(cycles, 10, 3),
                            shown(instructions, 10, 3),
                            shown(ipc, 6, 2),
                            shown(branches, 12, 5),
                            shown(misses, 12, 5),
                        );
                    }
                    println!();
                }
            }
        }
    }
    eprintln!("wrote {}", path.display());
    Ok(())
}

/// What a process counted: its passes, and each event's count, if
/// counted, and the share of the time it was counted, in percent; or why
/// its decoder failed.
type Counted = std::result::Result<(usize, Vec<(Option<u64>, f64)>), String>;

/// Counts `events` of `decoder` over `input` at `level` in a
/// `counters-sample` process under `perf stat`, its FIFOs and its counts'
/// file in `fifos`.
fn count(
    options: &Options,
    codec: &str,
    input: &Input,
    level: Level,
    decoder: &Decoder,
    events: &[&str; 2],
    fifos: &Path,
) -> Result<Counted> {
    let counts = fifos.join("counts");
    let (control, ack) = (fifos.join("control"), fifos.join("ack"));
    let mut command = match &options.cpu {
        Some(cpu) => {
            let mut command = Command::new("taskset");
            command.args(["-c", cpu, "perf"]);
            command
        },
        None => Command::new("perf"),
    };
    let fifo = format!("fifo:{},{}", control.display(), ack.display());
    let events: Vec<String> = events.iter().map(|event| format!("{event}:u")).collect();
    let output = command
        .args(["stat", "--delay", "-1", "--control", &fifo, "-x", ","])
        .arg("-o")
        .arg(&counts)
        .args(["-e", &events.join(",")])
        .arg("--")
        .arg(std::env::current_exe()?)
        .args(["counters-sample", codec, &input.name, &level.label()])
        .arg(options.piece.to_string())
        .arg(decoder.name)
        .arg(fifos)
        .output()?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("{}: {error}", input.name).into());
    }
    let stdout = String::from_utf8(output.stdout)?;
    let line = stdout.lines().last().ok_or("counters-sample: no answer")?;
    let passes = match serde_json::from_str(line)? {
        Line::Counted { passes } => passes,
        Line::Failure { failure } => return Ok(Err(failure)),
    };
    // Each line of `perf stat -x ,`: the count, its unit, the event, the
    // time it ran, and the share of it that it was counted, in percent.
    let text = fs::read_to_string(&counts)?;
    let mut counted = Vec::new();
    for event in &events {
        let fields = text
            .lines()
            .map(|line| line.split(',').collect::<Vec<_>>())
            .find(|fields| fields.get(2) == Some(&event.as_str()))
            .ok_or_else(|| format!("perf stat: no count of {event}"))?;
        let count = fields.first().and_then(|count| count.parse().ok());
        let running = fields.get(4).and_then(|share| share.parse().ok());
        counted.push((count, running.unwrap_or(0.0)));
    }
    Ok(Ok((passes, counted)))
}
