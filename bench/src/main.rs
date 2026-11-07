// This is free and unencumbered software released into the public domain.

//! The harness that measures the `decode-*` and `compute-*` crates against
//! their counterparts, as `bench/METHOD.md` lays out.
//!
//! It fetches and verifies the corpus `corpus.toml` names, encodes it and
//! the synthetic inputs once with the reference encoders at their codecs'
//! levels, recording each in `packed.toml`, and decodes each with every
//! decoder of its codec, whole or in pieces, by the procedure of
//! `procedure`, checking every output against the original; and counts
//! what each allocates, by `memory`; and generates each decoder's
//! `BENCH.md` tables from the results, by `tables`; and measures the code
//! each decoder and algorithm adds to a minimal program, by `size`. The
//! other measures are to follow (`TODO.md`, BENCH-09 and 10).
//!
//! From `bench/`, `cargo run --release --`, then:
//!
//! - `fetch`: fetches the corpus into `bench/corpus/`, and verifies it.
//! - `encode [<codec>...] [--only <prefix>]`: encodes the inputs, or those
//!   whose names begin with the prefix, such as `synthetic` or `silesia`.
//! - `check [<codec>...] [--only <prefix>] [--piece <bytes>]`: decodes
//!   them once each with every decoder, whole unless a piece size is
//!   given, checking every output, and prints how fast each went.
//! - `run [<codec>...] [--only <prefix>] [--piece <bytes>] [--cpu <n>]`:
//!   the procedure, its samples written to `bench/results/`; on Linux,
//!   pinned to CPU `n` by `taskset`, if given.
//! - `memory [<codec>...] [--only <prefix>] [--piece <bytes>]`: decodes
//!   them once each with every decoder, counting the heap each allocates,
//!   by `memory`, with a Markdown table, and its results written to
//!   `bench/results/`.
//! - `tables [<codec>...]`: writes the tables of each codec's
//!   `lib/decode-<codec>/BENCH.md` from `bench/results/`, by `tables`, and
//!   the code size of every decoder's and `compute-*` crate's.
//!
//! - `hash [<algorithm>...] [--cpu <n>]`: the procedure over the checksums
//!   and hashes, by `throughput`, with a Markdown table of each.
//! - `size`: builds `bench/probe/` alone and with each of our decoders
//!   and algorithms, by `size`, with a Markdown table of the code each
//!   adds, and its results written to `bench/results/`.
//!
//! Every codec, `bzip2` and `zstd`, or every algorithm, such as `crc32` or
//! `sha256`, unless some are named. For the ceiling, build with
//! `RUSTFLAGS="-C target-cpu=native"` into another `--target-dir`, and for
//! our checksums with the CPU's instructions, with `--features
//! intrinsics`: the results record either, and name their file so.

mod corpus;
mod decoders;
mod hashes;
mod inputs;
mod machine;
mod measure;
mod memory;
mod procedure;
mod size;
mod tables;
mod throughput;

use decoders::Codec;
use hashes::Algorithm;
use inputs::Level;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// What a command is given.
pub struct Options {
    pub codecs: Vec<Codec>,
    pub only: String,
    pub piece: usize,
    pub cpu: Option<String>,
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_default();
    let codec = |name: &str| {
        Codec::ALL
            .into_iter()
            .find(|codec| codec.name() == name)
            .ok_or_else(|| format!("no codec {name}"))
    };
    if command == "sample" {
        // `sample <codec> <input> <level> <piece> <seed>`, from `run`.
        let mut next = || args.next().ok_or("sample: too few arguments");
        let (codec, input) = (codec(&next()?)?, next()?);
        let level = Level::parse(&next()?).ok_or("sample: no such level")?;
        let (piece, seed) = (next()?.parse()?, next()?.parse()?);
        return procedure::sample(codec, &input, level, piece, seed);
    }
    let algorithm = |name: &str| {
        Algorithm::ALL
            .into_iter()
            .find(|algorithm| algorithm.name() == name)
            .ok_or_else(|| format!("no algorithm {name}"))
    };
    if command == "hash-sample" {
        // `hash-sample <algorithm> <size> <piece> <seed>`, from `hash`.
        let mut next = || args.next().ok_or("hash-sample: too few arguments");
        let algorithm = algorithm(&next()?)?;
        let (size, piece, seed) = (next()?.parse()?, next()?.parse()?, next()?.parse()?);
        return throughput::sample(algorithm, size, piece, seed);
    }
    if command == "size" {
        return size::run(corpus::bench());
    }
    let mut names = Vec::new();
    let mut options = Options {
        codecs: Vec::new(),
        only: String::new(),
        piece: usize::MAX,
        cpu: None,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--only" => options.only = args.next().ok_or("--only needs a prefix")?,
            "--piece" => options.piece = args.next().ok_or("--piece needs a size")?.parse()?,
            "--cpu" => options.cpu = Some(args.next().ok_or("--cpu needs a CPU")?),
            name => names.push(name.to_owned()),
        }
    }
    if command == "hash" {
        let mut algorithms = names
            .iter()
            .map(|name| algorithm(name))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if algorithms.is_empty() {
            algorithms = Algorithm::ALL.to_vec();
        }
        return throughput::run(&options, &algorithms, corpus::bench());
    }
    for name in &names {
        options.codecs.push(codec(name)?);
    }
    if options.codecs.is_empty() {
        options.codecs = Codec::ALL.to_vec();
    }
    match command.as_str() {
        "fetch" => corpus::fetch(),
        "encode" => {
            let inputs = corpus::inputs(&options.only)?;
            for codec in options.codecs {
                corpus::encode(codec, &inputs)?;
            }
            Ok(())
        },
        "check" => check(&options),
        "run" => procedure::run(&options, corpus::bench()),
        "memory" => memory::run(&options, corpus::bench()),
        "tables" => tables::run(&options.codecs, corpus::bench()),
        _ => Err("commands: fetch, encode, check, run, memory, tables, hash, size".into()),
    }
}

/// Decodes every input once with every decoder of each codec, checking
/// each output, and prints a table of how fast each went.
fn check(options: &Options) -> Result<()> {
    let inputs = corpus::inputs(&options.only)?;
    for &codec in &options.codecs {
        let decoders = codec.decoders();
        for decoder in decoders {
            let (name, krate) = (decoder.name, decoder.krate);
            println!("{name}: `{krate}`, in {}", decoder.language);
        }
        print!("{:<32} {:>8} {:>8}", codec.name(), "level", "ratio");
        for decoder in decoders {
            print!(" {:>13}", decoder.name);
        }
        println!();
        for input in &inputs {
            let original = input.bytes()?;
            for level in codec.levels(original.len()) {
                let packed = input.encoded(codec, level)?;
                let ratio = original.len() as f64 / packed.len() as f64;
                print!("{:<32} {:>8} {ratio:>8.2}", input.name, level.label());
                for decoder in decoders {
                    match measure::once(decoder, &packed, &original, options.piece) {
                        Ok(rate) => print!(" {rate:>13.1}"),
                        Err(failure) => print!(" {:>13}", format!("({failure})")),
                    }
                }
                println!();
            }
        }
    }
    Ok(())
}
