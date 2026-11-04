# decode-lzma

`decode-lzma` is to be an LZMA and LZMA2 decoder, written clean room from the
LZMA SDK, which Igor Pavlov placed in the public domain, and dedicated to the
public domain: pure, safe Rust without dependencies, for `#![no_std]` with an
allocator, never panicking on any input, of the same sans-I/O shape as
`decode-zstd`. LZMA, the Lempel-Ziv-Markov chain algorithm of 7-Zip, compresses
7z archives, ZIP's method 14, `.lzma` files, SquashFS and initramfs images, and
EROFS (as MicroLZMA); LZMA2, LZMA in chunks that may be stored or reset, is the
codec of xz streams and 7z's default method. It is one of the `decode-*` crates,
clean-room, public-domain decoders of common codecs; this release reserves its
name, and exports nothing yet.
