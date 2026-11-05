// This is free and unencumbered software released into the public domain.

//! The harness that measures the `decode-*` crates against their
//! counterparts, as `bench/METHOD.md` lays out.
//!
//! So far it fetches and verifies the corpus `corpus.toml` names, encodes
//! it and the synthetic inputs once with the reference encoders at their
//! codecs' levels, recording each in `packed.toml`, and decodes each with
//! every decoder of its codec, whole or in pieces, checking every output
//! against the original, and prints the median MB/s of decoded output of
//! each. The procedure's processes and statistics, the other measures, and
//! the generated tables are to follow (`TODO.md`, BENCH-05 to 07).
//!
//! From `bench/`, `cargo run --release --`, then:
//!
//! - `fetch`: fetches the corpus into `bench/corpus/`, and verifies it.
//! - `encode [<codec>...] [--only <prefix>]`: encodes the inputs, or those
//!   whose names begin with the prefix, such as `synthetic` or `silesia`.
//! - `run [<codec>...] [--only <prefix>] [--piece <bytes>] [--quick]`:
//!   decodes them, whole unless a piece size is given, and with `--quick`,
//!   once each, untimed but for itself, to check every output.
//!
//! Every codec, `bzip2` and `zstd`, unless some are named.

mod corpus;
mod decoders;
mod inputs;
mod measure;

use decoders::Codec;
use measure::Budget;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// What a command is given.
struct Options {
    codecs: Vec<Codec>,
    only: String,
    piece: usize,
    budget: Budget,
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_default();
    let mut options = Options {
        codecs: Vec::new(),
        only: String::new(),
        piece: usize::MAX,
        budget: Budget::FULL,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--only" => options.only = args.next().ok_or("--only needs a prefix")?,
            "--piece" => options.piece = args.next().ok_or("--piece needs a size")?.parse()?,
            "--quick" => options.budget = Budget::QUICK,
            name => options.codecs.push(
                *Codec::ALL
                    .iter()
                    .find(|codec| codec.name() == name)
                    .ok_or_else(|| format!("no codec {name}"))?,
            ),
        }
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
        "run" => run(&options),
        _ => Err("commands: fetch, encode, run".into()),
    }
}

/// Decodes every input with every decoder of each codec, and prints a
/// table of their median MB/s.
fn run(options: &Options) -> Result<()> {
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
                    let (piece, budget) = (options.piece, &options.budget);
                    match measure::throughput(decoder, &packed, &original, piece, budget) {
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
