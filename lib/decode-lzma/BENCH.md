# decode-lzma benchmarks

The figures of `decode-lzma`'s decoder, alone and against the main
alternatives, measured by the protocol of this repository's
[`bench/METHOD.md`](../../bench/METHOD.md): the same inputs, machines,
and builds for every decoder, each figure a median with its 95%
confidence interval, and every output checked against the original.

The harness's `tables` command generates the figures below from its raw
data, which stays on the machines that measured it; raw LZMA2 is
measured within `decode-xz`'s.

## Counterparts

| Decoder | Crate | Language |
| --- | --- | --- |
| liblzma, the reference | `liblzma` | C |
| lzma-rust2 | `lzma-rust2` | Rust, with its `unsafe` optimizations |
| lzma-rs | `lzma-rs` | Rust |

## Inputs

The protocol's corpus and synthetic cases, encoded by liblzma 5.8.4 as
`.lzma` files at presets 0, 6, and 9e: a header of the properties and an
unknown size, which callers read, then LZMA ending at its end marker.

## Machines

64-bit x86 on a current AMD core and a current Intel core, each a
cloud instance's own, and on an older AMD core, a VM's; and 64-bit ARM
on Apple M4 and on a Neoverse server.

<!-- filer-bench: machines -->

Measured so far:

- AMD EPYC 9R45 (1 vCPU under EC2 Nitro), 1934852 kB of memory, Linux
  7.0.0-1017-aws; rustc 1.99.1 (58cf8f9c3 2026-10-13); cc (Ubuntu
  15.2.0-16ubuntu1) 15.2.0.
- ARM Neoverse-V3 (1 vCPU under EC2 Nitro), 1833580 kB of memory, Linux
  7.0.0-1017-aws; rustc 1.99.1 (58cf8f9c3 2026-10-13); cc (Ubuntu
  15.2.0-16ubuntu1) 15.2.0.
- Intel Xeon 6975P-C (1 vCPU under EC2 Nitro), 3902872 kB of memory, Linux
  7.0.0-1017-aws; rustc 1.99.1 (58cf8f9c3 2026-10-13); cc (Ubuntu
  15.2.0-16ubuntu1) 15.2.0.

<!-- filer-bench: end -->

## Whole streams

MB/s of decoded output, per machine: a row per input and level, a column
per decoder, and ours relative to the reference.

<!-- filer-bench: whole -->

<!-- filer-bench: end -->

## Streaming

MB/s with input and output in pieces of 4 KiB and of 64 KiB.

<!-- filer-bench: streaming -->

<!-- filer-bench: end -->

## Small streams

The time per stream of about 64 B, 1 KiB, and 16 KiB packed, the
decoder's construction included, in µs, per machine: a row per size and
level, a column per decoder, and the reference's time relative to ours.

<!-- filer-bench: small -->

<!-- filer-bench: end -->

## Memory

Peak heap and allocations per stream, and the size of the decoder
itself.

<!-- filer-bench: memory -->

### AMD EPYC 9R45 (1 vCPU under EC2 Nitro), whole

Measured 2026-11-21, at `b66411c`, `decode-lzma` 0.0.4.

The size of each decoder's value: ours 3896 B, liblzma 136 B, lzma-rust2
4112 B, lzma-rs 3680 B.

#### Synthetic

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| random | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| random | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| random | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |
| zeros | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| zeros | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| zeros | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |
| pattern | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| pattern | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| pattern | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |
| far | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| far | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| far | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |

#### Silesia

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| dickens | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| dickens | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| dickens | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| mozilla | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| mozilla | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| mozilla | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| mr | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| mr | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| mr | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| nci | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| nci | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| nci | 9e | 32784.0 KiB, 15 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 32787.2 KiB, 95 |
| ooffice | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| ooffice | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| ooffice | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| osdb | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| osdb | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| osdb | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.3 KiB, 94 |
| reymont | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| reymont | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| reymont | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| samba | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| samba | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| samba | 9e | 32784.0 KiB, 15 | 65572.2 KiB, 5 | 32784.1 KiB, 15 | 32787.2 KiB, 95 |
| sao | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| sao | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| sao | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| webster | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| webster | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| webster | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| xml | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| xml | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| xml | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| x-ray | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| x-ray | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| x-ray | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.3 KiB, 94 |

#### Large files

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| enwik8 | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| enwik8 | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| linux-6.12.tar | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| linux-6.12.tar | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| linux-6.12.tar | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.3 KiB, 96 |

#### Canterbury

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| alice29.txt | 6 | 268.0 KiB, 8 | 8224.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| alice29.txt | 9e | 268.0 KiB, 8 | 65572.2 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| asyoulik.txt | 0 | 140.0 KiB, 7 | 288.8 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| asyoulik.txt | 6 | 140.0 KiB, 7 | 8224.8 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| asyoulik.txt | 9e | 140.0 KiB, 7 | 65572.2 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| cp.html | 0 | 44.0 KiB, 5 | 288.8 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| cp.html | 6 | 44.0 KiB, 5 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| cp.html | 9e | 44.0 KiB, 5 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 47.3 KiB, 85 |
| fields.c | 0 | 28.0 KiB, 4 | 288.8 KiB, 5 | 140.1 KiB, 7 | 31.3 KiB, 84 |
| fields.c | 6 | 28.0 KiB, 4 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| fields.c | 9e | 28.0 KiB, 4 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| grammar.lsp | 0 | 16.0 KiB, 2 | 288.8 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| grammar.lsp | 6 | 16.0 KiB, 2 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| grammar.lsp | 9e | 16.0 KiB, 2 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 19.3 KiB, 82 |
| kennedy.xls | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| kennedy.xls | 6 | 1036.0 KiB, 10 | 8224.8 KiB, 5 | 2060.1 KiB, 11 | 1039.2 KiB, 90 |
| kennedy.xls | 9e | 1036.0 KiB, 10 | 65572.2 KiB, 5 | 2060.1 KiB, 11 | 1039.2 KiB, 90 |
| lcet10.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| lcet10.txt | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 524.1 KiB, 9 | 527.2 KiB, 89 |
| lcet10.txt | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 524.1 KiB, 9 | 527.3 KiB, 89 |
| plrabn12.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| plrabn12.txt | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| plrabn12.txt | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| ptt5 | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| ptt5 | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| ptt5 | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| sum | 0 | 76.0 KiB, 6 | 288.8 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| sum | 6 | 76.0 KiB, 6 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| sum | 9e | 76.0 KiB, 6 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 79.3 KiB, 86 |
| xargs.1 | 0 | 20.0 KiB, 3 | 288.8 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| xargs.1 | 6 | 20.0 KiB, 3 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| xargs.1 | 9e | 20.0 KiB, 3 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| E.coli | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| E.coli | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| E.coli | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| bible.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| bible.txt | 6 | 4108.0 KiB, 12 | 8224.8 KiB, 5 | 4108.1 KiB, 12 | 4111.3 KiB, 92 |
| bible.txt | 9e | 4108.0 KiB, 12 | 65572.2 KiB, 5 | 4108.1 KiB, 12 | 4111.3 KiB, 92 |
| world192.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| world192.txt | 6 | 4108.0 KiB, 12 | 8224.8 KiB, 5 | 4108.1 KiB, 12 | 4111.2 KiB, 92 |
| world192.txt | 9e | 4108.0 KiB, 12 | 65572.2 KiB, 5 | 4108.1 KiB, 12 | 4111.3 KiB, 92 |

#### Rust's executables

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustc | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| rustc | 6 | 1036.0 KiB, 10 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 1039.2 KiB, 90 |
| rustc | 9e | 1036.0 KiB, 10 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 1039.3 KiB, 90 |
| librustc_driver.so | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| librustc_driver.so | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| librustc_driver.so | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| cargo | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| cargo | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| cargo | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |

Ours relative to liblzma's peak: 0.93x at the median, and at most 1.00x.

### AMD EPYC 9R45 (1 vCPU under EC2 Nitro), in pieces of 4 KiB

Measured 2026-11-21, at `b66411c`, `decode-lzma` 0.0.4.

The size of each decoder's value: ours 3896 B, liblzma 136 B, lzma-rust2
4112 B, lzma-rs 3680 B.

#### Synthetic

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| random | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 4102 | 271.3 KiB, 88 |
| random | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8216.0 KiB, 2088 | 8207.3 KiB, 93 |
| random | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16408.0 KiB, 2089 | 8207.2 KiB, 93 |
| zeros | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| zeros | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| zeros | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |
| pattern | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| pattern | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| pattern | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.3 KiB, 93 |
| far | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 4102 | 271.3 KiB, 88 |
| far | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8216.1 KiB, 1050 | 8207.2 KiB, 93 |
| far | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 1051 | 8207.2 KiB, 93 |

#### Silesia

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| dickens | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| dickens | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| dickens | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| mozilla | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 42 | 271.2 KiB, 88 |
| mozilla | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 17 | 8207.2 KiB, 93 |
| mozilla | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 29 | 65555.2 KiB, 96 |
| mr | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| mr | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| mr | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| nci | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| nci | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| nci | 9e | 32784.0 KiB, 15 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 32787.1 KiB, 95 |
| ooffice | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| ooffice | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| ooffice | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| osdb | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| osdb | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| osdb | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| reymont | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| reymont | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| reymont | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| samba | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| samba | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| samba | 9e | 32784.0 KiB, 15 | 65572.2 KiB, 5 | 32784.1 KiB, 15 | 32787.1 KiB, 95 |
| sao | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| sao | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| sao | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| webster | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| webster | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| webster | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.1 KiB, 96 |
| xml | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| xml | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| xml | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| x-ray | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| x-ray | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| x-ray | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |

#### Large files

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| enwik8 | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| enwik8 | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| linux-6.12.tar | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| linux-6.12.tar | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| linux-6.12.tar | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |

#### Canterbury

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| alice29.txt | 6 | 268.0 KiB, 8 | 8224.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| alice29.txt | 9e | 268.0 KiB, 8 | 65572.2 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| asyoulik.txt | 0 | 140.0 KiB, 7 | 288.8 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| asyoulik.txt | 6 | 140.0 KiB, 7 | 8224.8 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| asyoulik.txt | 9e | 140.0 KiB, 7 | 65572.2 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| cp.html | 0 | 44.0 KiB, 5 | 288.8 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| cp.html | 6 | 44.0 KiB, 5 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| cp.html | 9e | 44.0 KiB, 5 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| fields.c | 0 | 28.0 KiB, 4 | 288.8 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| fields.c | 6 | 28.0 KiB, 4 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| fields.c | 9e | 28.0 KiB, 4 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| grammar.lsp | 0 | 16.0 KiB, 2 | 288.8 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| grammar.lsp | 6 | 16.0 KiB, 2 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| grammar.lsp | 9e | 16.0 KiB, 2 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| kennedy.xls | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| kennedy.xls | 6 | 1036.0 KiB, 10 | 8224.8 KiB, 5 | 2060.1 KiB, 11 | 1039.2 KiB, 90 |
| kennedy.xls | 9e | 1036.0 KiB, 10 | 65572.2 KiB, 5 | 2060.1 KiB, 11 | 1039.2 KiB, 90 |
| lcet10.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| lcet10.txt | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 524.1 KiB, 9 | 527.2 KiB, 89 |
| lcet10.txt | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 524.1 KiB, 9 | 527.2 KiB, 89 |
| plrabn12.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| plrabn12.txt | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| plrabn12.txt | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| ptt5 | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| ptt5 | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| ptt5 | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| sum | 0 | 76.0 KiB, 6 | 288.8 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| sum | 6 | 76.0 KiB, 6 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| sum | 9e | 76.0 KiB, 6 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| xargs.1 | 0 | 20.0 KiB, 3 | 288.8 KiB, 5 | 140.1 KiB, 7 | 23.2 KiB, 83 |
| xargs.1 | 6 | 20.0 KiB, 3 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| xargs.1 | 9e | 20.0 KiB, 3 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 23.2 KiB, 83 |
| E.coli | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| E.coli | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| E.coli | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| bible.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| bible.txt | 6 | 4108.0 KiB, 12 | 8224.8 KiB, 5 | 4108.1 KiB, 12 | 4111.2 KiB, 92 |
| bible.txt | 9e | 4108.0 KiB, 12 | 65572.2 KiB, 5 | 4108.1 KiB, 12 | 4111.2 KiB, 92 |
| world192.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| world192.txt | 6 | 4108.0 KiB, 12 | 8224.8 KiB, 5 | 4108.1 KiB, 12 | 4111.2 KiB, 92 |
| world192.txt | 9e | 4108.0 KiB, 12 | 65572.2 KiB, 5 | 4108.1 KiB, 12 | 4111.2 KiB, 92 |

#### Rust's executables

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustc | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| rustc | 6 | 1036.0 KiB, 10 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 1039.2 KiB, 90 |
| rustc | 9e | 1036.0 KiB, 10 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 1039.2 KiB, 90 |
| librustc_driver.so | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 14 | 271.2 KiB, 88 |
| librustc_driver.so | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| librustc_driver.so | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| cargo | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 56 | 271.2 KiB, 88 |
| cargo | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| cargo | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 36 | 65555.2 KiB, 96 |

Ours relative to liblzma's peak: 0.93x at the median, and at most 1.00x.

### AMD EPYC 9R45 (1 vCPU under EC2 Nitro), in pieces of 64 KiB

Measured 2026-11-21, at `b66411c`, `decode-lzma` 0.0.4.

The size of each decoder's value: ours 3896 B, liblzma 136 B, lzma-rust2
4112 B, lzma-rs 3680 B.

#### Synthetic

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| random | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 262 | 271.2 KiB, 88 |
| random | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8216.0 KiB, 142 | 8207.2 KiB, 93 |
| random | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16408.0 KiB, 143 | 8207.2 KiB, 93 |
| zeros | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| zeros | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| zeros | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |
| pattern | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| pattern | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| pattern | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |
| far | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 262 | 271.2 KiB, 88 |
| far | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8216.0 KiB, 77 | 8207.2 KiB, 93 |
| far | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 78 | 8207.2 KiB, 93 |

#### Silesia

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| dickens | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| dickens | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| dickens | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| mozilla | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| mozilla | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| mozilla | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| mr | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| mr | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| mr | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| nci | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| nci | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| nci | 9e | 32784.0 KiB, 15 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 32787.2 KiB, 95 |
| ooffice | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| ooffice | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| ooffice | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| osdb | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| osdb | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| osdb | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.3 KiB, 94 |
| reymont | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| reymont | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| reymont | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| samba | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| samba | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| samba | 9e | 32784.0 KiB, 15 | 65572.2 KiB, 5 | 32784.1 KiB, 15 | 32787.2 KiB, 95 |
| sao | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| sao | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| sao | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| webster | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| webster | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| webster | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| xml | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| xml | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| xml | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| x-ray | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| x-ray | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| x-ray | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.3 KiB, 94 |

#### Large files

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| enwik8 | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| enwik8 | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| linux-6.12.tar | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| linux-6.12.tar | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| linux-6.12.tar | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.3 KiB, 96 |

#### Canterbury

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| alice29.txt | 6 | 268.0 KiB, 8 | 8224.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| alice29.txt | 9e | 268.0 KiB, 8 | 65572.2 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| asyoulik.txt | 0 | 140.0 KiB, 7 | 288.8 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| asyoulik.txt | 6 | 140.0 KiB, 7 | 8224.8 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| asyoulik.txt | 9e | 140.0 KiB, 7 | 65572.2 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| cp.html | 0 | 44.0 KiB, 5 | 288.8 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| cp.html | 6 | 44.0 KiB, 5 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| cp.html | 9e | 44.0 KiB, 5 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 47.3 KiB, 85 |
| fields.c | 0 | 28.0 KiB, 4 | 288.8 KiB, 5 | 140.1 KiB, 7 | 31.3 KiB, 84 |
| fields.c | 6 | 28.0 KiB, 4 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| fields.c | 9e | 28.0 KiB, 4 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| grammar.lsp | 0 | 16.0 KiB, 2 | 288.8 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| grammar.lsp | 6 | 16.0 KiB, 2 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| grammar.lsp | 9e | 16.0 KiB, 2 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 19.3 KiB, 82 |
| kennedy.xls | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| kennedy.xls | 6 | 1036.0 KiB, 10 | 8224.8 KiB, 5 | 2060.1 KiB, 11 | 1039.2 KiB, 90 |
| kennedy.xls | 9e | 1036.0 KiB, 10 | 65572.2 KiB, 5 | 2060.1 KiB, 11 | 1039.2 KiB, 90 |
| lcet10.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| lcet10.txt | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 524.1 KiB, 9 | 527.2 KiB, 89 |
| lcet10.txt | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 524.1 KiB, 9 | 527.3 KiB, 89 |
| plrabn12.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| plrabn12.txt | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| plrabn12.txt | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| ptt5 | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| ptt5 | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| ptt5 | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| sum | 0 | 76.0 KiB, 6 | 288.8 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| sum | 6 | 76.0 KiB, 6 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| sum | 9e | 76.0 KiB, 6 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 79.3 KiB, 86 |
| xargs.1 | 0 | 20.0 KiB, 3 | 288.8 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| xargs.1 | 6 | 20.0 KiB, 3 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| xargs.1 | 9e | 20.0 KiB, 3 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| E.coli | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| E.coli | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| E.coli | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| bible.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| bible.txt | 6 | 4108.0 KiB, 12 | 8224.8 KiB, 5 | 4108.1 KiB, 12 | 4111.3 KiB, 92 |
| bible.txt | 9e | 4108.0 KiB, 12 | 65572.2 KiB, 5 | 4108.1 KiB, 12 | 4111.3 KiB, 92 |
| world192.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| world192.txt | 6 | 4108.0 KiB, 12 | 8224.8 KiB, 5 | 4108.1 KiB, 12 | 4111.2 KiB, 92 |
| world192.txt | 9e | 4108.0 KiB, 12 | 65572.2 KiB, 5 | 4108.1 KiB, 12 | 4111.3 KiB, 92 |

#### Rust's executables

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustc | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| rustc | 6 | 1036.0 KiB, 10 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 1039.2 KiB, 90 |
| rustc | 9e | 1036.0 KiB, 10 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 1039.3 KiB, 90 |
| librustc_driver.so | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| librustc_driver.so | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| librustc_driver.so | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| cargo | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| cargo | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| cargo | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |

Ours relative to liblzma's peak: 0.93x at the median, and at most 1.00x.

### ARM Neoverse-V3 (1 vCPU under EC2 Nitro), whole

Measured 2026-11-21, at `b66411c`, `decode-lzma` 0.0.4.

The size of each decoder's value: ours 3896 B, liblzma 136 B, lzma-rust2
4112 B, lzma-rs 3680 B.

#### Synthetic

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| random | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| random | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| random | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |
| zeros | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| zeros | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| zeros | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |
| pattern | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| pattern | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| pattern | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |
| far | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| far | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| far | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |

#### Silesia

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| dickens | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| dickens | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| dickens | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| mozilla | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| mozilla | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| mozilla | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| mr | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| mr | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| mr | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| nci | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| nci | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| nci | 9e | 32784.0 KiB, 15 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 32787.2 KiB, 95 |
| ooffice | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| ooffice | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| ooffice | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| osdb | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| osdb | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| osdb | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.3 KiB, 94 |
| reymont | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| reymont | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| reymont | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| samba | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| samba | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| samba | 9e | 32784.0 KiB, 15 | 65572.2 KiB, 5 | 32784.1 KiB, 15 | 32787.2 KiB, 95 |
| sao | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| sao | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| sao | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| webster | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| webster | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| webster | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| xml | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| xml | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| xml | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| x-ray | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| x-ray | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| x-ray | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.3 KiB, 94 |

#### Large files

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| enwik8 | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| enwik8 | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| linux-6.12.tar | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| linux-6.12.tar | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| linux-6.12.tar | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.3 KiB, 96 |

#### Canterbury

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| alice29.txt | 6 | 268.0 KiB, 8 | 8224.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| alice29.txt | 9e | 268.0 KiB, 8 | 65572.2 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| asyoulik.txt | 0 | 140.0 KiB, 7 | 288.8 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| asyoulik.txt | 6 | 140.0 KiB, 7 | 8224.8 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| asyoulik.txt | 9e | 140.0 KiB, 7 | 65572.2 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| cp.html | 0 | 44.0 KiB, 5 | 288.8 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| cp.html | 6 | 44.0 KiB, 5 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| cp.html | 9e | 44.0 KiB, 5 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 47.3 KiB, 85 |
| fields.c | 0 | 28.0 KiB, 4 | 288.8 KiB, 5 | 140.1 KiB, 7 | 31.3 KiB, 84 |
| fields.c | 6 | 28.0 KiB, 4 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| fields.c | 9e | 28.0 KiB, 4 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| grammar.lsp | 0 | 16.0 KiB, 2 | 288.8 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| grammar.lsp | 6 | 16.0 KiB, 2 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| grammar.lsp | 9e | 16.0 KiB, 2 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 19.3 KiB, 82 |
| kennedy.xls | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| kennedy.xls | 6 | 1036.0 KiB, 10 | 8224.8 KiB, 5 | 2060.1 KiB, 11 | 1039.2 KiB, 90 |
| kennedy.xls | 9e | 1036.0 KiB, 10 | 65572.2 KiB, 5 | 2060.1 KiB, 11 | 1039.2 KiB, 90 |
| lcet10.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| lcet10.txt | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 524.1 KiB, 9 | 527.2 KiB, 89 |
| lcet10.txt | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 524.1 KiB, 9 | 527.3 KiB, 89 |
| plrabn12.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| plrabn12.txt | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| plrabn12.txt | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| ptt5 | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| ptt5 | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| ptt5 | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| sum | 0 | 76.0 KiB, 6 | 288.8 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| sum | 6 | 76.0 KiB, 6 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| sum | 9e | 76.0 KiB, 6 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 79.3 KiB, 86 |
| xargs.1 | 0 | 20.0 KiB, 3 | 288.8 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| xargs.1 | 6 | 20.0 KiB, 3 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| xargs.1 | 9e | 20.0 KiB, 3 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| E.coli | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| E.coli | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| E.coli | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| bible.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| bible.txt | 6 | 4108.0 KiB, 12 | 8224.8 KiB, 5 | 4108.1 KiB, 12 | 4111.3 KiB, 92 |
| bible.txt | 9e | 4108.0 KiB, 12 | 65572.2 KiB, 5 | 4108.1 KiB, 12 | 4111.3 KiB, 92 |
| world192.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| world192.txt | 6 | 4108.0 KiB, 12 | 8224.8 KiB, 5 | 4108.1 KiB, 12 | 4111.2 KiB, 92 |
| world192.txt | 9e | 4108.0 KiB, 12 | 65572.2 KiB, 5 | 4108.1 KiB, 12 | 4111.3 KiB, 92 |

#### Rust's executables

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustc | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| rustc | 6 | 1036.0 KiB, 10 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 1039.2 KiB, 90 |
| rustc | 9e | 1036.0 KiB, 10 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 1039.3 KiB, 90 |
| librustc_driver.so | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| librustc_driver.so | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| librustc_driver.so | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| cargo | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| cargo | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| cargo | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |

Ours relative to liblzma's peak: 0.93x at the median, and at most 1.00x.

### ARM Neoverse-V3 (1 vCPU under EC2 Nitro), in pieces of 4 KiB

Measured 2026-11-21, at `b66411c`, `decode-lzma` 0.0.4.

The size of each decoder's value: ours 3896 B, liblzma 136 B, lzma-rust2
4112 B, lzma-rs 3680 B.

#### Synthetic

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| random | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 4102 | 271.3 KiB, 88 |
| random | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8216.0 KiB, 2088 | 8207.3 KiB, 93 |
| random | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16408.0 KiB, 2089 | 8207.2 KiB, 93 |
| zeros | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| zeros | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| zeros | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |
| pattern | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| pattern | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| pattern | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.3 KiB, 93 |
| far | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 4102 | 271.3 KiB, 88 |
| far | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8216.1 KiB, 1050 | 8207.2 KiB, 93 |
| far | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 1051 | 8207.2 KiB, 93 |

#### Silesia

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| dickens | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| dickens | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| dickens | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| mozilla | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 42 | 271.2 KiB, 88 |
| mozilla | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 17 | 8207.2 KiB, 93 |
| mozilla | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 29 | 65555.2 KiB, 96 |
| mr | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| mr | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| mr | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| nci | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| nci | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| nci | 9e | 32784.0 KiB, 15 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 32787.1 KiB, 95 |
| ooffice | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| ooffice | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| ooffice | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| osdb | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| osdb | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| osdb | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| reymont | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| reymont | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| reymont | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| samba | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| samba | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| samba | 9e | 32784.0 KiB, 15 | 65572.2 KiB, 5 | 32784.1 KiB, 15 | 32787.1 KiB, 95 |
| sao | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| sao | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| sao | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| webster | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| webster | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| webster | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.1 KiB, 96 |
| xml | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| xml | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| xml | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| x-ray | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| x-ray | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| x-ray | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |

#### Large files

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| enwik8 | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| enwik8 | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| linux-6.12.tar | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| linux-6.12.tar | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| linux-6.12.tar | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |

#### Canterbury

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| alice29.txt | 6 | 268.0 KiB, 8 | 8224.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| alice29.txt | 9e | 268.0 KiB, 8 | 65572.2 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| asyoulik.txt | 0 | 140.0 KiB, 7 | 288.8 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| asyoulik.txt | 6 | 140.0 KiB, 7 | 8224.8 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| asyoulik.txt | 9e | 140.0 KiB, 7 | 65572.2 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| cp.html | 0 | 44.0 KiB, 5 | 288.8 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| cp.html | 6 | 44.0 KiB, 5 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| cp.html | 9e | 44.0 KiB, 5 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| fields.c | 0 | 28.0 KiB, 4 | 288.8 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| fields.c | 6 | 28.0 KiB, 4 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| fields.c | 9e | 28.0 KiB, 4 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| grammar.lsp | 0 | 16.0 KiB, 2 | 288.8 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| grammar.lsp | 6 | 16.0 KiB, 2 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| grammar.lsp | 9e | 16.0 KiB, 2 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| kennedy.xls | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| kennedy.xls | 6 | 1036.0 KiB, 10 | 8224.8 KiB, 5 | 2060.1 KiB, 11 | 1039.2 KiB, 90 |
| kennedy.xls | 9e | 1036.0 KiB, 10 | 65572.2 KiB, 5 | 2060.1 KiB, 11 | 1039.2 KiB, 90 |
| lcet10.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| lcet10.txt | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 524.1 KiB, 9 | 527.2 KiB, 89 |
| lcet10.txt | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 524.1 KiB, 9 | 527.2 KiB, 89 |
| plrabn12.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| plrabn12.txt | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| plrabn12.txt | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| ptt5 | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| ptt5 | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| ptt5 | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| sum | 0 | 76.0 KiB, 6 | 288.8 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| sum | 6 | 76.0 KiB, 6 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| sum | 9e | 76.0 KiB, 6 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| xargs.1 | 0 | 20.0 KiB, 3 | 288.8 KiB, 5 | 140.1 KiB, 7 | 23.2 KiB, 83 |
| xargs.1 | 6 | 20.0 KiB, 3 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| xargs.1 | 9e | 20.0 KiB, 3 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 23.2 KiB, 83 |
| E.coli | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| E.coli | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| E.coli | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| bible.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| bible.txt | 6 | 4108.0 KiB, 12 | 8224.8 KiB, 5 | 4108.1 KiB, 12 | 4111.2 KiB, 92 |
| bible.txt | 9e | 4108.0 KiB, 12 | 65572.2 KiB, 5 | 4108.1 KiB, 12 | 4111.2 KiB, 92 |
| world192.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| world192.txt | 6 | 4108.0 KiB, 12 | 8224.8 KiB, 5 | 4108.1 KiB, 12 | 4111.2 KiB, 92 |
| world192.txt | 9e | 4108.0 KiB, 12 | 65572.2 KiB, 5 | 4108.1 KiB, 12 | 4111.2 KiB, 92 |

#### Rust's executables

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustc | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| rustc | 6 | 1036.0 KiB, 10 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 1039.2 KiB, 90 |
| rustc | 9e | 1036.0 KiB, 10 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 1039.2 KiB, 90 |
| librustc_driver.so | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 14 | 271.2 KiB, 88 |
| librustc_driver.so | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| librustc_driver.so | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| cargo | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 56 | 271.2 KiB, 88 |
| cargo | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| cargo | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 36 | 65555.2 KiB, 96 |

Ours relative to liblzma's peak: 0.93x at the median, and at most 1.00x.

### ARM Neoverse-V3 (1 vCPU under EC2 Nitro), in pieces of 64 KiB

Measured 2026-11-21, at `b66411c`, `decode-lzma` 0.0.4.

The size of each decoder's value: ours 3896 B, liblzma 136 B, lzma-rust2
4112 B, lzma-rs 3680 B.

#### Synthetic

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| random | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 262 | 271.2 KiB, 88 |
| random | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8216.0 KiB, 142 | 8207.2 KiB, 93 |
| random | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16408.0 KiB, 143 | 8207.2 KiB, 93 |
| zeros | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| zeros | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| zeros | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |
| pattern | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| pattern | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| pattern | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |
| far | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 262 | 271.2 KiB, 88 |
| far | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8216.0 KiB, 77 | 8207.2 KiB, 93 |
| far | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 78 | 8207.2 KiB, 93 |

#### Silesia

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| dickens | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| dickens | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| dickens | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| mozilla | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| mozilla | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| mozilla | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| mr | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| mr | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| mr | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| nci | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| nci | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| nci | 9e | 32784.0 KiB, 15 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 32787.2 KiB, 95 |
| ooffice | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| ooffice | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| ooffice | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| osdb | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| osdb | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| osdb | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.3 KiB, 94 |
| reymont | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| reymont | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| reymont | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| samba | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| samba | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| samba | 9e | 32784.0 KiB, 15 | 65572.2 KiB, 5 | 32784.1 KiB, 15 | 32787.2 KiB, 95 |
| sao | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| sao | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| sao | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| webster | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| webster | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| webster | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| xml | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| xml | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| xml | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| x-ray | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| x-ray | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| x-ray | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.3 KiB, 94 |

#### Large files

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| enwik8 | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| enwik8 | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| linux-6.12.tar | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| linux-6.12.tar | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| linux-6.12.tar | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.3 KiB, 96 |

#### Canterbury

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| alice29.txt | 6 | 268.0 KiB, 8 | 8224.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| alice29.txt | 9e | 268.0 KiB, 8 | 65572.2 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| asyoulik.txt | 0 | 140.0 KiB, 7 | 288.8 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| asyoulik.txt | 6 | 140.0 KiB, 7 | 8224.8 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| asyoulik.txt | 9e | 140.0 KiB, 7 | 65572.2 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| cp.html | 0 | 44.0 KiB, 5 | 288.8 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| cp.html | 6 | 44.0 KiB, 5 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| cp.html | 9e | 44.0 KiB, 5 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 47.3 KiB, 85 |
| fields.c | 0 | 28.0 KiB, 4 | 288.8 KiB, 5 | 140.1 KiB, 7 | 31.3 KiB, 84 |
| fields.c | 6 | 28.0 KiB, 4 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| fields.c | 9e | 28.0 KiB, 4 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| grammar.lsp | 0 | 16.0 KiB, 2 | 288.8 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| grammar.lsp | 6 | 16.0 KiB, 2 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| grammar.lsp | 9e | 16.0 KiB, 2 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 19.3 KiB, 82 |
| kennedy.xls | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| kennedy.xls | 6 | 1036.0 KiB, 10 | 8224.8 KiB, 5 | 2060.1 KiB, 11 | 1039.2 KiB, 90 |
| kennedy.xls | 9e | 1036.0 KiB, 10 | 65572.2 KiB, 5 | 2060.1 KiB, 11 | 1039.2 KiB, 90 |
| lcet10.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| lcet10.txt | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 524.1 KiB, 9 | 527.2 KiB, 89 |
| lcet10.txt | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 524.1 KiB, 9 | 527.3 KiB, 89 |
| plrabn12.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| plrabn12.txt | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| plrabn12.txt | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| ptt5 | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| ptt5 | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| ptt5 | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| sum | 0 | 76.0 KiB, 6 | 288.8 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| sum | 6 | 76.0 KiB, 6 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| sum | 9e | 76.0 KiB, 6 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 79.3 KiB, 86 |
| xargs.1 | 0 | 20.0 KiB, 3 | 288.8 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| xargs.1 | 6 | 20.0 KiB, 3 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| xargs.1 | 9e | 20.0 KiB, 3 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| E.coli | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| E.coli | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| E.coli | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| bible.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| bible.txt | 6 | 4108.0 KiB, 12 | 8224.8 KiB, 5 | 4108.1 KiB, 12 | 4111.3 KiB, 92 |
| bible.txt | 9e | 4108.0 KiB, 12 | 65572.2 KiB, 5 | 4108.1 KiB, 12 | 4111.3 KiB, 92 |
| world192.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| world192.txt | 6 | 4108.0 KiB, 12 | 8224.8 KiB, 5 | 4108.1 KiB, 12 | 4111.2 KiB, 92 |
| world192.txt | 9e | 4108.0 KiB, 12 | 65572.2 KiB, 5 | 4108.1 KiB, 12 | 4111.3 KiB, 92 |

#### Rust's executables

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustc | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| rustc | 6 | 1036.0 KiB, 10 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 1039.2 KiB, 90 |
| rustc | 9e | 1036.0 KiB, 10 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 1039.3 KiB, 90 |
| librustc_driver.so | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| librustc_driver.so | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| librustc_driver.so | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| cargo | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| cargo | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| cargo | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |

Ours relative to liblzma's peak: 0.93x at the median, and at most 1.00x.

### Intel Xeon 6975P-C (1 vCPU under EC2 Nitro), whole

Measured 2026-11-21, at `b66411c`, `decode-lzma` 0.0.4.

The size of each decoder's value: ours 3896 B, liblzma 136 B, lzma-rust2
4112 B, lzma-rs 3680 B.

#### Synthetic

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| random | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| random | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| random | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |
| zeros | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| zeros | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| zeros | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |
| pattern | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| pattern | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| pattern | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |
| far | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| far | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| far | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |

#### Silesia

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| dickens | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| dickens | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| dickens | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| mozilla | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| mozilla | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| mozilla | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| mr | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| mr | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| mr | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| nci | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| nci | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| nci | 9e | 32784.0 KiB, 15 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 32787.2 KiB, 95 |
| ooffice | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| ooffice | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| ooffice | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| osdb | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| osdb | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| osdb | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.3 KiB, 94 |
| reymont | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| reymont | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| reymont | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| samba | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| samba | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| samba | 9e | 32784.0 KiB, 15 | 65572.2 KiB, 5 | 32784.1 KiB, 15 | 32787.2 KiB, 95 |
| sao | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| sao | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| sao | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| webster | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| webster | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| webster | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| xml | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| xml | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| xml | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| x-ray | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| x-ray | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| x-ray | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.3 KiB, 94 |

#### Large files

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| enwik8 | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| enwik8 | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| linux-6.12.tar | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| linux-6.12.tar | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| linux-6.12.tar | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.3 KiB, 96 |

#### Canterbury

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| alice29.txt | 6 | 268.0 KiB, 8 | 8224.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| alice29.txt | 9e | 268.0 KiB, 8 | 65572.2 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| asyoulik.txt | 0 | 140.0 KiB, 7 | 288.8 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| asyoulik.txt | 6 | 140.0 KiB, 7 | 8224.8 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| asyoulik.txt | 9e | 140.0 KiB, 7 | 65572.2 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| cp.html | 0 | 44.0 KiB, 5 | 288.8 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| cp.html | 6 | 44.0 KiB, 5 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| cp.html | 9e | 44.0 KiB, 5 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 47.3 KiB, 85 |
| fields.c | 0 | 28.0 KiB, 4 | 288.8 KiB, 5 | 140.1 KiB, 7 | 31.3 KiB, 84 |
| fields.c | 6 | 28.0 KiB, 4 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| fields.c | 9e | 28.0 KiB, 4 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| grammar.lsp | 0 | 16.0 KiB, 2 | 288.8 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| grammar.lsp | 6 | 16.0 KiB, 2 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| grammar.lsp | 9e | 16.0 KiB, 2 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 19.3 KiB, 82 |
| kennedy.xls | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| kennedy.xls | 6 | 1036.0 KiB, 10 | 8224.8 KiB, 5 | 2060.1 KiB, 11 | 1039.2 KiB, 90 |
| kennedy.xls | 9e | 1036.0 KiB, 10 | 65572.2 KiB, 5 | 2060.1 KiB, 11 | 1039.2 KiB, 90 |
| lcet10.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| lcet10.txt | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 524.1 KiB, 9 | 527.2 KiB, 89 |
| lcet10.txt | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 524.1 KiB, 9 | 527.3 KiB, 89 |
| plrabn12.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| plrabn12.txt | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| plrabn12.txt | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| ptt5 | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| ptt5 | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| ptt5 | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| sum | 0 | 76.0 KiB, 6 | 288.8 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| sum | 6 | 76.0 KiB, 6 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| sum | 9e | 76.0 KiB, 6 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 79.3 KiB, 86 |
| xargs.1 | 0 | 20.0 KiB, 3 | 288.8 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| xargs.1 | 6 | 20.0 KiB, 3 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| xargs.1 | 9e | 20.0 KiB, 3 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| E.coli | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| E.coli | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| E.coli | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| bible.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| bible.txt | 6 | 4108.0 KiB, 12 | 8224.8 KiB, 5 | 4108.1 KiB, 12 | 4111.3 KiB, 92 |
| bible.txt | 9e | 4108.0 KiB, 12 | 65572.2 KiB, 5 | 4108.1 KiB, 12 | 4111.3 KiB, 92 |
| world192.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| world192.txt | 6 | 4108.0 KiB, 12 | 8224.8 KiB, 5 | 4108.1 KiB, 12 | 4111.2 KiB, 92 |
| world192.txt | 9e | 4108.0 KiB, 12 | 65572.2 KiB, 5 | 4108.1 KiB, 12 | 4111.3 KiB, 92 |

#### Rust's executables

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustc | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| rustc | 6 | 1036.0 KiB, 10 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 1039.2 KiB, 90 |
| rustc | 9e | 1036.0 KiB, 10 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 1039.3 KiB, 90 |
| librustc_driver.so | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| librustc_driver.so | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| librustc_driver.so | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| cargo | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| cargo | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| cargo | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |

Ours relative to liblzma's peak: 0.93x at the median, and at most 1.00x.

### Intel Xeon 6975P-C (1 vCPU under EC2 Nitro), in pieces of 4 KiB

Measured 2026-11-21, at `b66411c`, `decode-lzma` 0.0.4.

The size of each decoder's value: ours 3896 B, liblzma 136 B, lzma-rust2
4112 B, lzma-rs 3680 B.

#### Synthetic

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| random | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 4102 | 271.3 KiB, 88 |
| random | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8216.0 KiB, 2088 | 8207.3 KiB, 93 |
| random | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16408.0 KiB, 2089 | 8207.2 KiB, 93 |
| zeros | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| zeros | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| zeros | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |
| pattern | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| pattern | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| pattern | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.3 KiB, 93 |
| far | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 4102 | 271.3 KiB, 88 |
| far | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8216.1 KiB, 1050 | 8207.2 KiB, 93 |
| far | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 1051 | 8207.2 KiB, 93 |

#### Silesia

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| dickens | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| dickens | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| dickens | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| mozilla | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 42 | 271.2 KiB, 88 |
| mozilla | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 17 | 8207.2 KiB, 93 |
| mozilla | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 29 | 65555.2 KiB, 96 |
| mr | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| mr | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| mr | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| nci | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| nci | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| nci | 9e | 32784.0 KiB, 15 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 32787.1 KiB, 95 |
| ooffice | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| ooffice | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| ooffice | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| osdb | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| osdb | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| osdb | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| reymont | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| reymont | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| reymont | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| samba | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| samba | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| samba | 9e | 32784.0 KiB, 15 | 65572.2 KiB, 5 | 32784.1 KiB, 15 | 32787.1 KiB, 95 |
| sao | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| sao | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| sao | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| webster | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| webster | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| webster | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.1 KiB, 96 |
| xml | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| xml | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| xml | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| x-ray | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| x-ray | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| x-ray | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |

#### Large files

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| enwik8 | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| enwik8 | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| linux-6.12.tar | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| linux-6.12.tar | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| linux-6.12.tar | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |

#### Canterbury

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| alice29.txt | 6 | 268.0 KiB, 8 | 8224.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| alice29.txt | 9e | 268.0 KiB, 8 | 65572.2 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| asyoulik.txt | 0 | 140.0 KiB, 7 | 288.8 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| asyoulik.txt | 6 | 140.0 KiB, 7 | 8224.8 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| asyoulik.txt | 9e | 140.0 KiB, 7 | 65572.2 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| cp.html | 0 | 44.0 KiB, 5 | 288.8 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| cp.html | 6 | 44.0 KiB, 5 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| cp.html | 9e | 44.0 KiB, 5 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| fields.c | 0 | 28.0 KiB, 4 | 288.8 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| fields.c | 6 | 28.0 KiB, 4 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| fields.c | 9e | 28.0 KiB, 4 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| grammar.lsp | 0 | 16.0 KiB, 2 | 288.8 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| grammar.lsp | 6 | 16.0 KiB, 2 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| grammar.lsp | 9e | 16.0 KiB, 2 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| kennedy.xls | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| kennedy.xls | 6 | 1036.0 KiB, 10 | 8224.8 KiB, 5 | 2060.1 KiB, 11 | 1039.2 KiB, 90 |
| kennedy.xls | 9e | 1036.0 KiB, 10 | 65572.2 KiB, 5 | 2060.1 KiB, 11 | 1039.2 KiB, 90 |
| lcet10.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| lcet10.txt | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 524.1 KiB, 9 | 527.2 KiB, 89 |
| lcet10.txt | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 524.1 KiB, 9 | 527.2 KiB, 89 |
| plrabn12.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| plrabn12.txt | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| plrabn12.txt | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| ptt5 | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| ptt5 | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| ptt5 | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| sum | 0 | 76.0 KiB, 6 | 288.8 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| sum | 6 | 76.0 KiB, 6 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| sum | 9e | 76.0 KiB, 6 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| xargs.1 | 0 | 20.0 KiB, 3 | 288.8 KiB, 5 | 140.1 KiB, 7 | 23.2 KiB, 83 |
| xargs.1 | 6 | 20.0 KiB, 3 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| xargs.1 | 9e | 20.0 KiB, 3 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 23.2 KiB, 83 |
| E.coli | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| E.coli | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| E.coli | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| bible.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| bible.txt | 6 | 4108.0 KiB, 12 | 8224.8 KiB, 5 | 4108.1 KiB, 12 | 4111.2 KiB, 92 |
| bible.txt | 9e | 4108.0 KiB, 12 | 65572.2 KiB, 5 | 4108.1 KiB, 12 | 4111.2 KiB, 92 |
| world192.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| world192.txt | 6 | 4108.0 KiB, 12 | 8224.8 KiB, 5 | 4108.1 KiB, 12 | 4111.2 KiB, 92 |
| world192.txt | 9e | 4108.0 KiB, 12 | 65572.2 KiB, 5 | 4108.1 KiB, 12 | 4111.2 KiB, 92 |

#### Rust's executables

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustc | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| rustc | 6 | 1036.0 KiB, 10 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 1039.2 KiB, 90 |
| rustc | 9e | 1036.0 KiB, 10 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 1039.2 KiB, 90 |
| librustc_driver.so | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 14 | 271.2 KiB, 88 |
| librustc_driver.so | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| librustc_driver.so | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| cargo | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 56 | 271.2 KiB, 88 |
| cargo | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| cargo | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 36 | 65555.2 KiB, 96 |

Ours relative to liblzma's peak: 0.93x at the median, and at most 1.00x.

### Intel Xeon 6975P-C (1 vCPU under EC2 Nitro), in pieces of 64 KiB

Measured 2026-11-21, at `b66411c`, `decode-lzma` 0.0.4.

The size of each decoder's value: ours 3896 B, liblzma 136 B, lzma-rust2
4112 B, lzma-rs 3680 B.

#### Synthetic

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| random | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 262 | 271.2 KiB, 88 |
| random | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8216.0 KiB, 142 | 8207.2 KiB, 93 |
| random | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16408.0 KiB, 143 | 8207.2 KiB, 93 |
| zeros | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| zeros | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| zeros | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |
| pattern | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| pattern | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| pattern | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 8207.2 KiB, 93 |
| far | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 284.1 KiB, 262 | 271.2 KiB, 88 |
| far | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8216.0 KiB, 77 | 8207.2 KiB, 93 |
| far | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 78 | 8207.2 KiB, 93 |

#### Silesia

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| dickens | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| dickens | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| dickens | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| mozilla | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| mozilla | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| mozilla | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| mr | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| mr | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| mr | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.2 KiB, 94 |
| nci | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| nci | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| nci | 9e | 32784.0 KiB, 15 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 32787.2 KiB, 95 |
| ooffice | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| ooffice | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| ooffice | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| osdb | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| osdb | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| osdb | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.3 KiB, 94 |
| reymont | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| reymont | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| reymont | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| samba | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| samba | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| samba | 9e | 32784.0 KiB, 15 | 65572.2 KiB, 5 | 32784.1 KiB, 15 | 32787.2 KiB, 95 |
| sao | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| sao | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| sao | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| webster | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| webster | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| webster | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| xml | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| xml | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| xml | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| x-ray | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| x-ray | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| x-ray | 9e | 16396.0 KiB, 14 | 65572.2 KiB, 5 | 16396.1 KiB, 14 | 16399.3 KiB, 94 |

#### Large files

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| enwik8 | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| enwik8 | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| linux-6.12.tar | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| linux-6.12.tar | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| linux-6.12.tar | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.3 KiB, 96 |

#### Canterbury

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| alice29.txt | 6 | 268.0 KiB, 8 | 8224.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| alice29.txt | 9e | 268.0 KiB, 8 | 65572.2 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| asyoulik.txt | 0 | 140.0 KiB, 7 | 288.8 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| asyoulik.txt | 6 | 140.0 KiB, 7 | 8224.8 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| asyoulik.txt | 9e | 140.0 KiB, 7 | 65572.2 KiB, 5 | 268.1 KiB, 8 | 143.2 KiB, 87 |
| cp.html | 0 | 44.0 KiB, 5 | 288.8 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| cp.html | 6 | 44.0 KiB, 5 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 47.2 KiB, 85 |
| cp.html | 9e | 44.0 KiB, 5 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 47.3 KiB, 85 |
| fields.c | 0 | 28.0 KiB, 4 | 288.8 KiB, 5 | 140.1 KiB, 7 | 31.3 KiB, 84 |
| fields.c | 6 | 28.0 KiB, 4 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| fields.c | 9e | 28.0 KiB, 4 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 31.2 KiB, 84 |
| grammar.lsp | 0 | 16.0 KiB, 2 | 288.8 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| grammar.lsp | 6 | 16.0 KiB, 2 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 19.2 KiB, 82 |
| grammar.lsp | 9e | 16.0 KiB, 2 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 19.3 KiB, 82 |
| kennedy.xls | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| kennedy.xls | 6 | 1036.0 KiB, 10 | 8224.8 KiB, 5 | 2060.1 KiB, 11 | 1039.2 KiB, 90 |
| kennedy.xls | 9e | 1036.0 KiB, 10 | 65572.2 KiB, 5 | 2060.1 KiB, 11 | 1039.2 KiB, 90 |
| lcet10.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| lcet10.txt | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 524.1 KiB, 9 | 527.2 KiB, 89 |
| lcet10.txt | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 524.1 KiB, 9 | 527.3 KiB, 89 |
| plrabn12.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| plrabn12.txt | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| plrabn12.txt | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| ptt5 | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.2 KiB, 88 |
| ptt5 | 6 | 524.0 KiB, 9 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| ptt5 | 9e | 524.0 KiB, 9 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 527.2 KiB, 89 |
| sum | 0 | 76.0 KiB, 6 | 288.8 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| sum | 6 | 76.0 KiB, 6 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 79.2 KiB, 86 |
| sum | 9e | 76.0 KiB, 6 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 79.3 KiB, 86 |
| xargs.1 | 0 | 20.0 KiB, 3 | 288.8 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| xargs.1 | 6 | 20.0 KiB, 3 | 8224.8 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| xargs.1 | 9e | 20.0 KiB, 3 | 65572.2 KiB, 5 | 140.1 KiB, 7 | 23.3 KiB, 83 |
| E.coli | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| E.coli | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| E.coli | 9e | 8204.0 KiB, 13 | 65572.2 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| bible.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| bible.txt | 6 | 4108.0 KiB, 12 | 8224.8 KiB, 5 | 4108.1 KiB, 12 | 4111.3 KiB, 92 |
| bible.txt | 9e | 4108.0 KiB, 12 | 65572.2 KiB, 5 | 4108.1 KiB, 12 | 4111.3 KiB, 92 |
| world192.txt | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| world192.txt | 6 | 4108.0 KiB, 12 | 8224.8 KiB, 5 | 4108.1 KiB, 12 | 4111.2 KiB, 92 |
| world192.txt | 9e | 4108.0 KiB, 12 | 65572.2 KiB, 5 | 4108.1 KiB, 12 | 4111.3 KiB, 92 |

#### Rust's executables

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustc | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| rustc | 6 | 1036.0 KiB, 10 | 8224.8 KiB, 5 | 1036.1 KiB, 10 | 1039.2 KiB, 90 |
| rustc | 9e | 1036.0 KiB, 10 | 65572.2 KiB, 5 | 1036.1 KiB, 10 | 1039.3 KiB, 90 |
| librustc_driver.so | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| librustc_driver.so | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.3 KiB, 93 |
| librustc_driver.so | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |
| cargo | 0 | 268.0 KiB, 8 | 288.8 KiB, 5 | 268.1 KiB, 8 | 271.3 KiB, 88 |
| cargo | 6 | 8204.0 KiB, 13 | 8224.8 KiB, 5 | 8204.1 KiB, 13 | 8207.2 KiB, 93 |
| cargo | 9e | 65552.0 KiB, 16 | 65572.2 KiB, 5 | 65552.1 KiB, 16 | 65555.2 KiB, 96 |

Ours relative to liblzma's peak: 0.93x at the median, and at most 1.00x.

<!-- filer-bench: end -->

## Checkpoints

The time to take a checkpoint and to resume from one, and the bytes each
holds, in streams decoded in pieces of 32 KiB, per machine: a row per
input and level, with our decoder's µs to take one and to resume from
one, each the median over several processes of the median over the
places they were taken at, and the most heap one held and its
allocations. No counterpart offers them.

<!-- filer-bench: checkpoints -->

<!-- filer-bench: end -->

## Code size

The bytes of `.text` the decoder adds to a minimal `#![no_std]` program
that decodes a stream with it, from construction to `finish`, built
portable with fat LTO; read-only data, such as tables, is not counted.

<!-- filer-bench: size -->

| Target | `opt-level` 3 | `opt-level` "s" |
| --- | ---: | ---: |
| aarch64-apple-darwin | 18556 | 15428 |
| aarch64-unknown-linux-gnu | 21976 | 15264 |
| x86_64-unknown-linux-gnu | 25689 | 18553 |

On aarch64-apple-darwin, measured 2026-11-07, at `86d2427`,
`decode-lzma` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On aarch64-unknown-linux-gnu, measured 2026-11-16, at `d73cac4`,
`decode-lzma` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On x86_64-unknown-linux-gnu, measured 2026-11-16, at `d73cac4`,
`decode-lzma` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

<!-- filer-bench: end -->
