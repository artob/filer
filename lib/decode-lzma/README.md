# decode-lzma

An LZMA and LZMA2 decoder, ported from the LZMA SDK, which Igor Pavlov placed
in the public domain, and dedicated to the public domain. It decodes LZMA, the
Lempel-Ziv-Markov chain algorithm of 7-Zip, as 7z archives, ZIP's method 14,
and `.lzma` files hold it, and LZMA2, LZMA in chunks that may be stored or
reset, as xz streams and 7z archives hold it. It consumes a stream to its end
and no further, so that a container reads on from what follows; it never
compresses. It is pure, safe Rust without dependencies, builds with
`#![no_std]` and an allocator, never panics on any input, and refuses what the
LZMA SDK refuses, with a dictionary that grows only as the stream does.

Its `Decoder` is sans-I/O: `decode` takes what input there is, fills what
room there is, and says how much of each it used, so it serves blocking,
async, and in-memory callers alike. `checkpoint` offers a copy of the
decoder anywhere, for resuming a long stream part way. It is one of the
`decode-*` crates: clean-room, public-domain decoders of common codecs,
each of the same shape.

```rust
use decode_lzma::{Decoder, Error};

/// Decodes a whole `.lzma` file in memory: its properties, its size, and
/// the stream.
fn decompress(file: &[u8]) -> Result<Vec<u8>, Error> {
    let (header, stream) = file.split_at_checked(13).ok_or(Error::Corrupt)?;
    let properties = header[..5].try_into().unwrap();
    let size = u64::from_le_bytes(header[5..].try_into().unwrap());
    // All ones, for a size not known, which an end marker ends.
    let size = (size != u64::MAX).then_some(size);
    let mut decoder = Decoder::lzma(properties, size)?;
    let (mut output, mut buf, mut at) = (Vec::new(), [0; 4096], 0);
    loop {
        let (consumed, produced) = decoder.decode(&stream[at..], &mut buf)?;
        at += consumed;
        output.extend_from_slice(&buf[..produced]);
        if (consumed, produced) == (0, 0) {
            break;
        }
    }
    // Checks that the stream ended where a stream may.
    decoder.finish()?;
    Ok(output)
}

fn main() -> Result<(), Error> {
    // `printf hello | xz --format=lzma -0`.
    let file = [
        0x5D, 0x00, 0x00, 0x04, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0x00, 0x34, 0x19, 0x49, 0xDB, 0x85, 0x64, 0xF1, 0x93, 0xB1, 0xFF,
        0xFB, 0x8F, 0xC0, 0x00,
    ];
    assert_eq!(decompress(&file)?, b"hello");
    Ok(())
}
```

This is free and unencumbered software released into the public domain.
