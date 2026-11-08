// This is free and unencumbered software released into the public domain.

//! The code each decoder and algorithm of ours adds to a minimal program,
//! as `METHOD.md` asks (item 7): `size` builds `bench/probe/`, a
//! `#![no_std]` static library whose one function decodes a stream, or
//! computes a checksum or hash, with what its feature enables, alone and
//! with each feature, at `opt-level` 3 and `"s"`; its fat LTO keeps only
//! the code that function needs. Each build's `.text`, summed over its
//! objects, less the baseline's, is what the decoder or algorithm adds;
//! read-only data, such as a CRC's tables, is not counted. Sizes are
//! deterministic, so one build of each suffices. The probe is built for
//! this machine's target, portable, and without dependencies: without
//! `intrinsics`, on by default (D66), whose dispatch is not what is
//! measured, and without the Rust flags of the harness's own build.

use crate::{Result, hashes::Algorithm, machine::Machine};
use object::{Object as _, ObjectSection as _, SectionKind, read::archive::ArchiveFile};
use serde::Serialize;
use std::{fs, io::Write as _, path::Path, process::Command};

/// Our decoders, by crate, each the probe's feature of that name.
pub const DECODERS: [&str; 9] = [
    "decode-bzip2",
    "decode-deflate",
    "decode-gzip",
    "decode-lz4",
    "decode-lzma",
    "decode-lzo",
    "decode-xz",
    "decode-zlib",
    "decode-zstd",
];

/// The `opt-level`s built at.
pub const LEVELS: [&str; 2] = ["3", "s"];

/// What is measured: each decoder, by its crate, then each algorithm, by
/// the harness's name, as `(crate, feature)`.
pub fn subjects() -> Vec<(&'static str, &'static str)> {
    let decoders = DECODERS.into_iter().map(|krate| (krate, krate));
    let algorithms = Algorithm::ALL.map(|algorithm| (algorithm.krate(), algorithm.name()));
    decoders.chain(algorithms).collect()
}

/// A line of the results' file, after the machine's.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Record<'a> {
    Machine(&'a Machine),
    Size {
        #[serde(rename = "crate")]
        krate: &'a str,
        subject: &'a str,
        /// The target triple, the `opt-level`, and the bytes of `.text` it
        /// adds to the baseline's, which is also recorded.
        target: &'a str,
        level: &'a str,
        bytes: i64,
        baseline: u64,
    },
}

/// The `size` command: builds the probe alone and with each subject at
/// each level, prints a table of the bytes each adds, and writes them to
/// `bench/results/`.
pub fn run(bench: &Path) -> Result<()> {
    let mut machine = Machine::now(bench);
    // Built as `build` builds the probe, whatever the harness's build was.
    (machine.rustflags, machine.features) = (String::new(), Vec::new());
    machine.rustc = output("rustc", &["--version"])?.trim().to_owned();
    let host = output("rustc", &["-vV"])?;
    let target = host
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or("rustc -vV: no host")?
        .to_owned();
    let subjects = subjects();
    let mut sizes = Vec::new();
    for level in LEVELS {
        let baseline = build(bench, level, None)?;
        for &(krate, subject) in &subjects {
            let text = build(bench, level, Some(subject))?;
            sizes.push((
                krate,
                subject,
                level,
                i64::try_from(text)? - i64::try_from(baseline)?,
                baseline,
            ));
        }
    }
    // The probe's lock names the version of every crate it linked.
    let lock = fs::read_to_string(bench.join("probe").join("Cargo.lock"))?;
    let mut name = None;
    for line in lock.lines() {
        if let Some(value) = line.strip_prefix("name = ") {
            name = Some(value.trim_matches('"').to_owned());
        } else if let (Some(value), Some(name)) = (line.strip_prefix("version = "), name.take()) {
            let package = format!("{name} {}", value.trim_matches('"'));
            if !machine.packages.contains(&package) {
                machine.packages.push(package);
            }
        }
    }
    let (path, mut results) = machine.create(bench)?;
    writeln!(
        results,
        "{}",
        serde_json::to_string(&Record::Machine(&machine))?
    )?;
    for &(krate, subject, level, bytes, baseline) in &sizes {
        let record = Record::Size {
            krate,
            subject,
            target: &target,
            level,
            bytes,
            baseline,
        };
        writeln!(results, "{}", serde_json::to_string(&record)?)?;
    }
    println!("On {target}, the bytes of `.text` each adds:");
    println!();
    println!("| Crate | Subject | `opt-level` 3 | `opt-level` \"s\" |");
    println!("| --- | --- | ---: | ---: |");
    for &(krate, subject) in &subjects {
        print!("| `{krate}` | {subject} |");
        for level in LEVELS {
            let found = sizes
                .iter()
                .find(|size| (size.1, size.2) == (subject, level));
            match found {
                Some(size) => print!(" {} |", size.3),
                None => print!(" |"),
            }
        }
        println!();
    }
    eprintln!("wrote {}", path.display());
    Ok(())
}

/// What `program` prints with `args`, if it succeeds.
fn output(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        return Err(format!("{program} failed").into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

/// Builds the probe at `level`, with `feature`, if any, and returns the
/// bytes of `.text` of every object it holds.
fn build(bench: &Path, level: &str, feature: Option<&str>) -> Result<u64> {
    let probe = bench.join("probe");
    let target = bench.join("target").join("probe");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .args(["build", "--release", "--quiet", "--manifest-path"])
        .arg(probe.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", &target)
        .env("CARGO_PROFILE_RELEASE_OPT_LEVEL", level)
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS");
    if let Some(feature) = feature {
        command.args(["--features", feature]);
    }
    if !command.status()?.success() {
        return Err(format!("the probe with {feature:?} failed to build").into());
    }
    let bytes = fs::read(target.join("release").join("libfiler_bench_probe.a"))?;
    let archive = ArchiveFile::parse(&*bytes)?;
    let mut text = 0;
    for member in archive.members() {
        let data = member?.data(&*bytes)?;
        // Every member of a static library is an object.
        let object = object::File::parse(data)?;
        let sections = object
            .sections()
            .filter(|section| section.kind() == SectionKind::Text);
        text += sections.map(|section| section.size()).sum::<u64>();
    }
    Ok(text)
}
