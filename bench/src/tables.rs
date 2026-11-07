// This is free and unencumbered software released into the public domain.

//! The tables of each `decode-*` crate's `BENCH.md`, and the code size of
//! each `compute-*` crate's, generated from the raw data in
//! `bench/results/`, never edited by hand, as `METHOD.md` asks.
//!
//! `tables` reads every run there and keeps those made at a commit from a
//! clean tree, as only of those can it be told what they measured. Of each
//! machine, build, codec, way of feeding, input, and level, and of each
//! target, crate, subject, and `opt-level` sized, it takes the newest run
//! that measured it, and computes its figures as `run` does,
//! with the same bootstrap from the same seed, so that the same data always
//! makes the same tables. It writes them between the `BENCH.md`'s markers,
//! `<!-- filer-bench: <section> -->` and `<!-- filer-bench: end -->`,
//! leaving the rest as it is.

use crate::{
    Result, corpus,
    decoders::{Codec, Decoder},
    inputs::{Level, SplitMix},
    machine::Machine,
    procedure::{self, Figures},
    size,
};
use serde::Deserialize;
use std::{collections::BTreeMap, fmt::Write as _, fs, path::Path};

/// A line of a results' file, as `procedure`, `memory`, and `size` write
/// it.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Record {
    Machine(Box<Machine>),
    Sample {
        codec: String,
        input: String,
        level: String,
        piece: Option<usize>,
        bytes: usize,
        #[serde(default)]
        packed: Option<usize>,
        process: usize,
        decoder: String,
        ns: u64,
    },
    Failure {
        codec: String,
        input: String,
        level: String,
        #[serde(default)]
        piece: Option<usize>,
        decoder: String,
        failure: String,
    },
    Memory {
        codec: String,
        input: String,
        level: String,
        piece: Option<usize>,
        decoder: String,
        value: usize,
        peak: Option<u64>,
        allocations: Option<u64>,
    },
    Size {
        #[serde(rename = "crate")]
        krate: String,
        subject: String,
        target: String,
        level: String,
        bytes: i64,
    },
    #[serde(other)]
    Other,
}

/// What a figure is of: the machine, whether the build was for it alone,
/// the codec, the pieces fed, the input, and the level.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    machine: String,
    native: bool,
    codec: String,
    piece: Option<usize>,
    input: String,
    level: String,
}

/// What a run timed of a key: the input's size, packed and not, each
/// decoder's samples by process, and why any failed.
#[derive(Default)]
struct Timed {
    bytes: usize,
    packed: Option<usize>,
    samples: BTreeMap<String, BTreeMap<usize, Vec<f64>>>,
    failures: BTreeMap<String, String>,
}

/// What a run counted of a key, by decoder: the decoder's value, its
/// peak and allocations, if counted, or why it failed.
type Counted = BTreeMap<String, std::result::Result<(usize, Option<(u64, u64)>), String>>;

/// What a size is of: the target, the crate, the subject, and the
/// `opt-level`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Sized {
    target: String,
    krate: String,
    subject: String,
    level: String,
}

/// The figures kept, each with the run it came from.
#[derive(Default)]
struct Data {
    runs: Vec<Machine>,
    timed: BTreeMap<Key, (usize, Timed)>,
    counted: BTreeMap<Key, (usize, Counted)>,
    sized: BTreeMap<Sized, (usize, i64)>,
}

/// The `tables` command: writes the tables of each codec's `BENCH.md`,
/// `lib/decode-<codec>/BENCH.md`, and the code size of every crate that
/// `size` measures, from `bench/results/`.
pub fn run(codecs: &[Codec], bench: &Path) -> Result<()> {
    let data = read(&bench.join("results"))?;
    let order: Vec<String> = corpus::inputs("")?
        .into_iter()
        .map(|input| input.name)
        .collect();
    for &codec in codecs {
        let path = bench
            .join("..")
            .join("lib")
            .join(codec.decoders()[0].krate)
            .join("BENCH.md");
        let mut text = fs::read_to_string(&path)?;
        let sections = [
            ("machines", machines(&data, codec)),
            ("whole", timing(&data, codec, &order, &[None])),
            ("streaming", timing(&data, codec, &order, &PIECES)),
            ("memory", memory(&data, codec, &order)),
        ];
        for (name, body) in sections {
            text = replace(&text, name, &body?)?;
        }
        fs::write(&path, text)?;
        eprintln!("wrote {}", path.display());
    }
    let mut krates: Vec<&str> = size::subjects().iter().map(|&(krate, _)| krate).collect();
    krates.dedup();
    for krate in krates {
        let path = bench.join("..").join("lib").join(krate).join("BENCH.md");
        let text = fs::read_to_string(&path)?;
        fs::write(&path, replace(&text, "size", &sizes(&data, krate)?)?)?;
        eprintln!("wrote {}", path.display());
    }
    Ok(())
}

/// The pieces of streaming, as `METHOD.md` names them.
const PIECES: [Option<usize>; 2] = [Some(4 << 10), Some(64 << 10)];

/// Reads every run in `results`, oldest first, keeping of each key the
/// newest run's figures.
fn read(results: &Path) -> Result<Data> {
    let mut paths: Vec<_> = fs::read_dir(results)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::result::Result<_, _>>()?;
    paths.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension == "jsonl")
    });
    let mut runs = Vec::new();
    for path in paths {
        let text = fs::read_to_string(&path)?;
        let mut lines = text.lines();
        let Some(Ok(Record::Machine(machine))) = lines.next().map(serde_json::from_str) else {
            continue;
        };
        if machine.commit.is_empty() || machine.dirty {
            continue;
        }
        runs.push((*machine, lines.map(str::to_owned).collect::<Vec<_>>()));
    }
    runs.sort_by(|a, b| a.0.date.cmp(&b.0.date));
    let mut data = Data::default();
    for (run, (machine, lines)) in runs.into_iter().enumerate() {
        let native = machine.rustflags.contains("target-cpu=native");
        let key = |codec: String, input: String, level: String, piece| Key {
            machine: label(&machine),
            native,
            codec,
            piece,
            input,
            level,
        };
        // The checksums' runs, whose lines are `throughput`'s, hold none of
        // these.
        let records: Vec<Record> = lines
            .iter()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        // A run either times or counts, and its failures are of that.
        let counts = records
            .iter()
            .any(|record| matches!(record, Record::Memory { .. }));
        let mut timed: BTreeMap<Key, Timed> = BTreeMap::new();
        let mut counted: BTreeMap<Key, Counted> = BTreeMap::new();
        for record in records {
            match record {
                Record::Sample {
                    codec,
                    input,
                    level,
                    piece,
                    bytes,
                    packed,
                    process,
                    decoder,
                    ns,
                } => {
                    let timed = timed.entry(key(codec, input, level, piece)).or_default();
                    (timed.bytes, timed.packed) = (bytes, packed);
                    let processes = timed.samples.entry(decoder).or_default();
                    processes.entry(process).or_default().push(ns as f64);
                },
                Record::Failure {
                    codec,
                    input,
                    level,
                    piece,
                    decoder,
                    failure,
                } => {
                    let key = key(codec, input, level, piece);
                    match counts {
                        true => {
                            let counted = counted.entry(key).or_default();
                            counted.insert(decoder, Err(failure));
                        },
                        false => {
                            let timed = timed.entry(key).or_default();
                            timed.failures.insert(decoder, failure);
                        },
                    }
                },
                Record::Memory {
                    codec,
                    input,
                    level,
                    piece,
                    decoder,
                    value,
                    peak,
                    allocations,
                } => {
                    let counted = counted.entry(key(codec, input, level, piece)).or_default();
                    counted.insert(decoder, Ok((value, peak.zip(allocations))));
                },
                Record::Size {
                    krate,
                    subject,
                    target,
                    level,
                    bytes,
                } => {
                    let key = Sized {
                        target,
                        krate,
                        subject,
                        level,
                    };
                    data.sized.insert(key, (run, bytes));
                },
                Record::Machine(_) | Record::Other => {},
            }
        }
        for (key, timed) in timed {
            if !timed.samples.is_empty() || !timed.failures.is_empty() {
                data.timed.insert(key, (run, timed));
            }
        }
        for (key, counted) in counted {
            data.counted.insert(key, (run, counted));
        }
        data.runs.push(machine);
    }
    Ok(data)
}

/// A machine's name in tables: its CPU, and whether it is virtual.
fn label(machine: &Machine) -> String {
    let how = match machine.virtualized.as_str() {
        "none" | "" => format!("{} cores, bare metal", machine.cores),
        hypervisor => format!(
            "{} vCPUs under {}",
            machine.cores,
            hypervisor.to_uppercase()
        ),
    };
    format!("{} ({how})", machine.cpu)
}

/// The version of `krate` that the harness links now, or, of one of ours
/// it does not link, the repository's.
fn version(krate: &str) -> Option<&'static str> {
    let packages = env!("FILER_BENCH_PACKAGES").split(';');
    let linked = packages
        .filter_map(|package| package.split_once(' '))
        .find(|&(name, _)| name == krate)
        .map(|(_, version)| version);
    linked.or_else(|| {
        size::subjects()
            .iter()
            .any(|&(k, _)| k == krate)
            .then(|| include_str!("../../VERSION").trim())
    })
}

/// When and at what the runs of `runs` measured `krate`: each run's date,
/// commit, and version, marked if older than the crate's.
fn provenance(data: &Data, krate: &str, runs: impl IntoIterator<Item = usize>) -> String {
    let mut seen: Vec<String> = Vec::new();
    for run in runs {
        let machine = &data.runs[run];
        let ran = machine
            .packages
            .iter()
            .filter_map(|package| package.split_once(' '))
            .find(|&(name, _)| name == krate)
            .map(|(_, version)| version);
        let mut note = format!(
            "{}, at `{}`",
            machine.date.get(..10).unwrap_or(&machine.date),
            machine.commit.get(..7).unwrap_or(&machine.commit)
        );
        if let Some(ran) = ran {
            write!(note, ", `{krate}` {ran}").ok();
            if version(krate).is_some_and(|now| now != ran) {
                note.push_str(", older than this release");
            }
        }
        if !seen.contains(&note) {
            seen.push(note);
        }
    }
    seen.join("; ")
}

/// The `machines` section: each machine measured, and how.
fn machines(data: &Data, codec: Codec) -> Result<String> {
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    let timed = data.timed.iter().map(|(key, (run, _))| (key, *run));
    let counted = data.counted.iter().map(|(key, (run, _))| (key, *run));
    for (_, run) in timed
        .chain(counted)
        .filter(|(key, _)| key.codec == codec.name())
    {
        let machine = &data.runs[run];
        let state = match machine.governor.as_str() {
            "" => String::new(),
            governor => format!(", the `{governor}` governor"),
        };
        seen.entry(label(machine)).or_insert(format!(
            "{}, {} of memory, {}{state}; {}; {}.",
            label(machine),
            machine.memory,
            machine.kernel,
            machine.rustc,
            machine.cc,
        ));
    }
    if seen.is_empty() {
        return Ok("Measured so far: none.".into());
    }
    let mut text = String::from("Measured so far:\n\n");
    for line in seen.values() {
        writeln!(text, "- {}", wrap(line, 2).trim_start())?;
    }
    Ok(text.trim_end().into())
}

/// The inputs of `codec` that `keys` hold, by set, in the corpus's order.
fn sets<'a>(
    order: &[String],
    keys: impl Iterator<Item = &'a Key>,
) -> Vec<(&'static str, Vec<String>)> {
    let mut present: Vec<&String> = keys.map(|key| &key.input).collect();
    present.sort();
    present.dedup();
    let mut sets: Vec<(&'static str, Vec<String>)> = Vec::new();
    for name in order.iter().filter(|name| present.contains(name)) {
        let title = match name.split_once('/').map(|(set, _)| set) {
            Some("synthetic") => "Synthetic",
            Some("silesia") => "Silesia",
            Some("canterbury") => "Canterbury",
            Some("rust") => "Rust's executables",
            _ => "Large files",
        };
        match sets.iter_mut().find(|(t, _)| *t == title) {
            Some((_, names)) => names.push(name.clone()),
            None => sets.push((title, vec![name.clone()])),
        }
    }
    sets
}

/// The levels of `keys`, in order.
fn levels<'a>(keys: impl Iterator<Item = &'a Key>) -> Vec<String> {
    let mut levels: Vec<Level> = keys.filter_map(|key| Level::parse(&key.level)).collect();
    levels.sort_by_key(|level| (level.number, level.long));
    levels.dedup();
    levels.iter().map(|level| level.label()).collect()
}

/// How pieces were fed, in headings.
fn fed(piece: Option<usize>) -> String {
    match piece {
        None => "whole".into(),
        Some(piece) => format!("in pieces of {} KiB", piece >> 10),
    }
}

/// The figures of `decoder` in `timed`: its median of each process, in
/// the processes' order, or why it failed.
fn figures(timed: &Timed, decoder: &Decoder) -> Figures {
    let medians = timed.samples.get(decoder.name).map(|processes| {
        let mut processes: Vec<Vec<f64>> = processes.values().cloned().collect();
        processes
            .iter_mut()
            .map(|times| procedure::median(times))
            .collect()
    });
    Figures {
        medians: medians.unwrap_or_default(),
        failure: timed.failures.get(decoder.name).cloned(),
    }
}

/// A table of each decoder's MB/s, its interval, and ours relative to the
/// reference, per machine, build, and way of feeding of `pieces`, a table
/// per set of inputs; then ours relative to each counterpart, per level.
fn timing(data: &Data, codec: Codec, order: &[String], pieces: &[Option<usize>]) -> Result<String> {
    let decoders = codec.decoders();
    let reference = &decoders[1];
    let mut text = String::new();
    let mut groups: Vec<(String, bool, Option<usize>)> = data
        .timed
        .keys()
        .filter(|key| key.codec == codec.name() && pieces.contains(&key.piece))
        .map(|key| (key.machine.clone(), key.native, key.piece))
        .collect();
    groups.sort();
    groups.dedup();
    for (machine, native, piece) in groups {
        let of = |key: &&Key| {
            key.codec == codec.name()
                && key.machine == machine
                && key.native == native
                && key.piece == piece
        };
        let keys: Vec<&Key> = data.timed.keys().filter(of).collect();
        let build = match native {
            true => ", built for it alone",
            false => "",
        };
        let piece_heading = match piece {
            None => String::new(),
            Some(_) => format!(", {}", fed(piece)),
        };
        writeln!(text, "### {machine}{build}{piece_heading}\n")?;
        let runs = keys.iter().map(|key| data.timed[*key].0);
        writeln!(
            text,
            "{}\n",
            wrap(
                &format!(
                    "Measured {}.",
                    provenance(data, codec.decoders()[0].krate, runs)
                ),
                0
            )
        )?;
        // Ours relative to each counterpart, by level, over every input.
        let mut ratios: Vec<(String, usize, f64)> = Vec::new();
        let levels = levels(keys.iter().copied());
        for (title, inputs) in sets(order, keys.iter().copied()) {
            writeln!(text, "#### {title}\n")?;
            write!(text, "| Input | Level | Ratio |")?;
            for decoder in decoders {
                write!(text, " {} |", decoder.name)?;
            }
            writeln!(text, " ours ÷ {} |", reference.name)?;
            write!(text, "| --- | ---: | ---: |")?;
            for _ in decoders {
                write!(text, " ---: |")?;
            }
            writeln!(text, " ---: |")?;
            for input in &inputs {
                for level in &levels {
                    let key = Key {
                        machine: machine.clone(),
                        native,
                        codec: codec.name().into(),
                        piece,
                        input: input.clone(),
                        level: level.clone(),
                    };
                    let Some((_, timed)) = data.timed.get(&key) else {
                        continue;
                    };
                    let name = input
                        .split_once('/')
                        .map_or(input.as_str(), |(_, name)| name);
                    let ratio = match timed.packed {
                        Some(packed) => format!("{:.2}", timed.bytes as f64 / packed as f64),
                        None => "?".into(),
                    };
                    write!(text, "| {name} | {level} | {ratio} |")?;
                    let all: Vec<Figures> = decoders.iter().map(|d| figures(timed, d)).collect();
                    for (i, figures) in all.iter().enumerate() {
                        if let Some(failure) = &figures.failure {
                            write!(text, " ({}) |", failure.replace('|', "/"))?;
                            continue;
                        }
                        if figures.medians.is_empty() {
                            write!(text, " |")?;
                            continue;
                        }
                        let mut random = SplitMix(0x5EED);
                        let (median, low, high) =
                            procedure::bootstrap(&figures.medians, &mut random);
                        let width = (high - low) / 2.0 / median * 100.0;
                        let rate = timed.bytes as f64 / median * 1e3;
                        write!(text, " {rate:.1} ± {width:.1}% |")?;
                        // Their speed relative to ours, inverted.
                        if let Some(theirs) = procedure::paired(&all[0], figures).filter(|_| i > 0)
                        {
                            ratios.push((level.clone(), i, 1.0 / theirs));
                        }
                    }
                    match procedure::paired(&all[0], &all[1]) {
                        Some(theirs) => writeln!(text, " {:.2}x |", 1.0 / theirs)?,
                        None => writeln!(text, " |")?,
                    }
                }
            }
            writeln!(text)?;
        }
        writeln!(
            text,
            "Ours relative to each counterpart, the geometric mean over inputs:\n"
        )?;
        write!(text, "| Level |")?;
        for decoder in &decoders[1..] {
            write!(text, " {} |", decoder.name)?;
        }
        writeln!(text)?;
        write!(text, "| ---: |")?;
        for _ in &decoders[1..] {
            write!(text, " ---: |")?;
        }
        writeln!(text)?;
        for level in &levels {
            write!(text, "| {level} |")?;
            for i in 1..decoders.len() {
                let logs: Vec<f64> = ratios
                    .iter()
                    .filter(|(l, d, _)| l == level && *d == i)
                    .map(|&(.., ratio)| ratio.ln())
                    .collect();
                match logs.is_empty() {
                    true => write!(text, " |")?,
                    false => {
                        let mean = (logs.iter().sum::<f64>() / logs.len() as f64).exp();
                        write!(text, " {mean:.2}x |")?;
                    },
                }
            }
            writeln!(text)?;
        }
        writeln!(text)?;
    }
    match text.is_empty() {
        true => Ok("Not measured yet.".into()),
        false => Ok(text.trim_end().into()),
    }
}

/// A table of the most heap each decoder held and its allocations, per
/// machine and way of feeding, a table per set of inputs; then ours
/// relative to the reference's peak.
fn memory(data: &Data, codec: Codec, order: &[String]) -> Result<String> {
    let decoders = codec.decoders();
    let reference = &decoders[1];
    let mut text = String::new();
    let mut groups: Vec<(String, bool, Option<usize>)> = data
        .counted
        .keys()
        .filter(|key| key.codec == codec.name())
        .map(|key| (key.machine.clone(), key.native, key.piece))
        .collect();
    groups.sort();
    groups.dedup();
    for (machine, native, piece) in groups {
        let of = |key: &&Key| {
            key.codec == codec.name()
                && key.machine == machine
                && key.native == native
                && key.piece == piece
        };
        let keys: Vec<&Key> = data.counted.keys().filter(of).collect();
        let build = match native {
            true => ", built for it alone",
            false => "",
        };
        writeln!(text, "### {machine}{build}, {}\n", fed(piece))?;
        let runs = keys.iter().map(|key| data.counted[*key].0);
        writeln!(
            text,
            "{}\n",
            wrap(
                &format!(
                    "Measured {}.",
                    provenance(data, codec.decoders()[0].krate, runs)
                ),
                0
            )
        )?;
        // The size of each decoder's value, as any key records it.
        let values: Vec<String> = decoders
            .iter()
            .filter_map(|decoder| {
                let value =
                    keys.iter()
                        .find_map(|key| match data.counted[*key].1.get(decoder.name) {
                            Some(Ok((value, _))) => Some(*value),
                            _ => None,
                        });
                value.map(|value| format!("{} {value} B", decoder.name))
            })
            .collect();
        writeln!(
            text,
            "{}\n",
            wrap(
                &format!("The size of each decoder's value: {}.", values.join(", ")),
                0
            )
        )?;
        let mut ratios: Vec<f64> = Vec::new();
        let levels = levels(keys.iter().copied());
        for (title, inputs) in sets(order, keys.iter().copied()) {
            writeln!(text, "#### {title}\n")?;
            write!(text, "| Input | Level |")?;
            for decoder in decoders {
                write!(text, " {} |", decoder.name)?;
            }
            writeln!(text)?;
            write!(text, "| --- | ---: |")?;
            for _ in decoders {
                write!(text, " ---: |")?;
            }
            writeln!(text)?;
            for input in &inputs {
                for level in &levels {
                    let key = Key {
                        machine: machine.clone(),
                        native,
                        codec: codec.name().into(),
                        piece,
                        input: input.clone(),
                        level: level.clone(),
                    };
                    let Some((_, counted)) = data.counted.get(&key) else {
                        continue;
                    };
                    let name = input
                        .split_once('/')
                        .map_or(input.as_str(), |(_, name)| name);
                    write!(text, "| {name} | {level} |")?;
                    for decoder in decoders {
                        match counted.get(decoder.name) {
                            None => write!(text, " |")?,
                            Some(Err(failure)) => {
                                write!(text, " ({}) |", failure.replace('|', "/"))?
                            },
                            Some(Ok((_, None))) => write!(text, " (not counted) |")?,
                            Some(Ok((_, Some((peak, allocations))))) => {
                                write!(text, " {:.1} KiB, {allocations} |", *peak as f64 / 1024.0)?
                            },
                        }
                    }
                    writeln!(text)?;
                    let peak = |name: &str| match counted.get(name) {
                        Some(Ok((_, Some((peak, _))))) if *peak > 0 => Some(*peak as f64),
                        _ => None,
                    };
                    if let (Some(ours), Some(theirs)) =
                        (peak(decoders[0].name), peak(reference.name))
                    {
                        ratios.push(ours / theirs);
                    }
                }
            }
            writeln!(text)?;
        }
        if !ratios.is_empty() {
            let most = ratios.iter().copied().fold(f64::MIN, f64::max);
            let median = procedure::median(&mut ratios);
            let summary = format!(
                "Ours relative to {}'s peak: {median:.2}x at the median, and at \
                 most {most:.2}x.",
                reference.name
            );
            writeln!(text, "{}\n", wrap(&summary, 0))?;
        }
    }
    match text.is_empty() {
        true => Ok("Not measured yet.".into()),
        false => Ok(text.trim_end().into()),
    }
}

/// A table of the bytes of `.text` each of `krate`'s subjects adds, per
/// target and `opt-level`, and when and with what each target's were
/// measured.
fn sizes(data: &Data, krate: &str) -> Result<String> {
    let keys: Vec<&Sized> = data.sized.keys().filter(|key| key.krate == krate).collect();
    if keys.is_empty() {
        return Ok("Not measured yet.".into());
    }
    let subjects: Vec<&str> = size::subjects()
        .into_iter()
        .filter(|&(k, subject)| k == krate && keys.iter().any(|key| key.subject == subject))
        .map(|(_, subject)| subject)
        .collect();
    let mut targets: Vec<&str> = keys.iter().map(|key| key.target.as_str()).collect();
    targets.sort();
    targets.dedup();
    // A decoder is its crate's one subject, and needs no column of its own.
    let named = subjects.len() > 1 || subjects.first().is_some_and(|&s| s != krate);
    let mut text = String::new();
    if named {
        write!(text, "| Algorithm ")?;
    }
    write!(text, "| Target |")?;
    for level in size::LEVELS {
        write!(text, " `opt-level` {} |", opt_level(level))?;
    }
    writeln!(text)?;
    if named {
        write!(text, "| --- ")?;
    }
    write!(text, "| --- |")?;
    for _ in size::LEVELS {
        write!(text, " ---: |")?;
    }
    writeln!(text)?;
    for subject in &subjects {
        for target in &targets {
            if named {
                write!(text, "| {subject} ")?;
            }
            write!(text, "| {target} |")?;
            for level in size::LEVELS {
                let key = Sized {
                    target: (*target).into(),
                    krate: krate.into(),
                    subject: (*subject).into(),
                    level: level.into(),
                };
                match data.sized.get(&key) {
                    Some((_, bytes)) => write!(text, " {bytes} |")?,
                    None => write!(text, " |")?,
                }
            }
            writeln!(text)?;
        }
    }
    for target in &targets {
        let runs: Vec<usize> = keys
            .iter()
            .filter(|key| key.target == *target)
            .map(|key| data.sized[*key].0)
            .collect();
        let mut rustc: Vec<&str> = runs
            .iter()
            .map(|&run| data.runs[run].rustc.as_str())
            .collect();
        rustc.sort();
        rustc.dedup();
        let line = format!(
            "On {target}, measured {}, by {}.",
            provenance(data, krate, runs),
            rustc.join(" and ")
        );
        writeln!(text, "\n{}", wrap(&line, 0))?;
    }
    Ok(text.trim_end().into())
}

/// An `opt-level` as `Cargo.toml` writes it.
fn opt_level(level: &str) -> String {
    match level.parse::<u8>() {
        Ok(_) => level.into(),
        Err(_) => format!("\"{level}\""),
    }
}

/// `text` wrapped at 72 columns, its lines after the first indented by
/// `indent`.
fn wrap(text: &str, indent: usize) -> String {
    let mut wrapped = String::new();
    let mut column = 0;
    for word in text.split_whitespace() {
        if column > 0 && column + 1 + word.len() > 72 {
            wrapped.push('\n');
            wrapped.push_str(&" ".repeat(indent));
            column = indent;
        } else if column > 0 {
            wrapped.push(' ');
            column += 1;
        }
        wrapped.push_str(word);
        column += word.len();
    }
    wrapped
}

/// `text` with what lies between its markers of section `name` replaced
/// by `body`.
fn replace(text: &str, name: &str, body: &str) -> Result<String> {
    let start = format!("<!-- filer-bench: {name} -->\n");
    let end = "<!-- filer-bench: end -->";
    let at = text
        .find(&start)
        .ok_or_else(|| format!("no marker of {name}"))?
        + start.len();
    let close = text[at..]
        .find(end)
        .ok_or_else(|| format!("{name}: no end marker"))?
        + at;
    Ok(format!("{}\n{body}\n\n{}", &text[..at], &text[close..]))
}
