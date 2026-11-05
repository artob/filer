// This is free and unencumbered software released into the public domain.

//! The harness that measures the `decode-*` crates against their
//! counterparts, as `bench/METHOD.md` lays out.
//!
//! So far it decodes the synthetic inputs, each encoded by the reference
//! encoder at its codec's levels, with every decoder of each codec, whole
//! or in pieces, checks every output against the original, and prints the
//! median MB/s of decoded output of each. The corpus, the procedure's
//! processes and statistics, the other measures, and the generated tables
//! are to follow (`TODO.md`, BENCH-01).
//!
//! `cargo run --release -- [bzip2] [zstd] [--piece <bytes>]`, from
//! `bench/`: every codec unless some are named, whole streams unless a
//! piece size is given.

mod decoders;
mod inputs;
mod measure;

use decoders::Codec;
use inputs::Packed;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut codecs, mut piece) = (Vec::new(), usize::MAX);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--piece" => piece = args.next().ok_or("--piece needs a size")?.parse()?,
            name => codecs.push(
                *Codec::ALL
                    .iter()
                    .find(|codec| codec.name() == name)
                    .ok_or_else(|| format!("no codec {name}"))?,
            ),
        }
    }
    if codecs.is_empty() {
        codecs = Codec::ALL.to_vec();
    }
    let inputs = inputs::synthetic();
    for codec in codecs {
        let decoders = codec.decoders();
        for decoder in decoders {
            let (name, krate) = (decoder.name, decoder.krate);
            println!("{name}: `{krate}`, in {}", decoder.language);
        }
        print!(
            "{:<8} {:<8} {:>5} {:>7}",
            codec.name(),
            "input",
            "level",
            "ratio"
        );
        for decoder in decoders {
            print!(" {:>13}", decoder.name);
        }
        println!();
        for input in &inputs {
            for &level in codec.levels() {
                let packed = Packed {
                    level,
                    bytes: codec.encode(&input.data, level)?,
                };
                let ratio = input.data.len() as f64 / packed.bytes.len() as f64;
                print!("{:<8} {:<8} {level:>5} {ratio:>7.2}", "", input.name);
                for decoder in decoders {
                    match measure::throughput(decoder, &packed, &input.data, piece) {
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
