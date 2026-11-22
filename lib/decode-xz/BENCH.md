# decode-xz benchmarks

The figures of `decode-xz`'s decoder, alone and against the main
alternatives, measured by the protocol of this repository's
[`bench/METHOD.md`](../../bench/METHOD.md): the same inputs, machines,
and builds for every decoder, each figure a median with its 95%
confidence interval, and every output checked against the original.

The harness's `tables` command generates the figures below from its raw
data, which stays on the machines that measured it; SHA-256 checks and
the x86 BCJ filter are not measured yet.

## Counterparts

| Decoder | Crate | Language |
| --- | --- | --- |
| liblzma, the reference | `liblzma` | C |
| lzma-rust2 | `lzma-rust2` | Rust, with its `unsafe` optimizations |
| lzma-rs | `lzma-rs` | Rust |
| xz4rust | `xz4rust` | Rust |

## Inputs

The protocol's corpus and synthetic cases, encoded by liblzma 5.8.4 at
presets 0, 6, and 9e, as xz writes them by default since 5.6: in its
multithreaded mode, in blocks of three times the dictionary and at least
1 MiB, the same on any number of threads, each checked by CRC-64.

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

Measured 2026-11-21, at `b66411c`, `decode-xz` 0.0.4.

The size of each decoder's value: ours 336 B, liblzma 136 B, lzma-rust2
4440 B, lzma-rs 0 B, xz4rust 8 B.

#### Synthetic

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| random | 0 | 261.4 KiB, 98 | 290.8 KiB, 16 | 257.2 KiB, 35 | 2921.7 KiB, 804 | 290.1 KiB, 3 |
| random | 6 | 8197.4 KiB, 20 | 8226.8 KiB, 9 | 8193.1 KiB, 11 | 23345.0 KiB, 227 | 8226.1 KiB, 3 |
| random | 9e | 16393.4 KiB, 21 | 65574.2 KiB, 9 | 8193.1 KiB, 11 | 23343.5 KiB, 227 | 65574.1 KiB, 3 |
| zeros | 0 | 271.9 KiB, 106 | 290.8 KiB, 16 | 269.2 KiB, 43 | 2063.6 KiB, 764 | 290.1 KiB, 3 |
| zeros | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 16399.4 KiB, 101 | 8226.1 KiB, 3 |
| zeros | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 16399.4 KiB, 101 | 65574.1 KiB, 3 |
| pattern | 0 | 271.9 KiB, 106 | 290.8 KiB, 16 | 269.2 KiB, 43 | 2063.6 KiB, 764 | 290.1 KiB, 3 |
| pattern | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 16399.4 KiB, 101 | 8226.1 KiB, 3 |
| pattern | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 16399.4 KiB, 101 | 65574.1 KiB, 3 |
| far | 0 | 261.4 KiB, 98 | 290.8 KiB, 16 | 257.2 KiB, 35 | 2921.6 KiB, 804 | 290.1 KiB, 3 |
| far | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 23353.9 KiB, 158 | 8226.1 KiB, 3 |
| far | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 23353.9 KiB, 158 | 65574.1 KiB, 3 |

#### Silesia

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| dickens | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2063.6 KiB, 955 | 290.1 KiB, 3 |
| dickens | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26353.0 KiB, 102 | 8226.1 KiB, 3 |
| dickens | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26352.9 KiB, 102 | 65574.1 KiB, 3 |
| mozilla | 0 | 271.9 KiB, 639 | 290.8 KiB, 57 | 270.1 KiB, 251 | 2945.7 KiB, 4653 | 290.1 KiB, 3 |
| mozilla | 6 | 8207.9 KiB, 56 | 8226.8 KiB, 11 | 8205.1 KiB, 29 | 57363.4 KiB, 299 | 8226.1 KiB, 3 |
| mozilla | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 115579.4 KiB, 105 | 65574.1 KiB, 3 |
| mr | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2063.6 KiB, 955 | 290.1 KiB, 3 |
| mr | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26136.3 KiB, 102 | 8226.1 KiB, 3 |
| mr | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26136.3 KiB, 102 | 65574.1 KiB, 3 |
| nci | 0 | 271.9 KiB, 418 | 290.8 KiB, 40 | 269.6 KiB, 165 | 2064.0 KiB, 3046 | 290.1 KiB, 3 |
| nci | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57363.4 KiB, 201 | 8226.1 KiB, 3 |
| nci | 9e | 32787.9 KiB, 23 | 65574.2 KiB, 9 | 32785.1 KiB, 14 | 65554.5 KiB, 103 | 65574.1 KiB, 3 |
| ooffice | 0 | 271.9 KiB, 80 | 290.8 KiB, 14 | 269.2 KiB, 33 | 2063.5 KiB, 574 | 290.1 KiB, 3 |
| ooffice | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 14215.4 KiB, 101 | 8226.1 KiB, 3 |
| ooffice | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 14215.4 KiB, 101 | 65574.1 KiB, 3 |
| osdb | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2063.6 KiB, 955 | 290.1 KiB, 3 |
| osdb | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26248.7 KiB, 102 | 8226.1 KiB, 3 |
| osdb | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26248.8 KiB, 102 | 65574.1 KiB, 3 |
| reymont | 0 | 271.9 KiB, 93 | 290.8 KiB, 15 | 269.2 KiB, 38 | 2063.6 KiB, 668 | 290.1 KiB, 3 |
| reymont | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 14679.3 KiB, 101 | 8226.1 KiB, 3 |
| reymont | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 14679.3 KiB, 101 | 65574.1 KiB, 3 |
| samba | 0 | 271.9 KiB, 275 | 290.8 KiB, 29 | 269.6 KiB, 110 | 2063.9 KiB, 2001 | 290.1 KiB, 3 |
| samba | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 53887.4 KiB, 103 | 8226.1 KiB, 3 |
| samba | 9e | 32787.9 KiB, 23 | 65574.2 KiB, 9 | 32785.1 KiB, 14 | 53887.4 KiB, 103 | 65574.1 KiB, 3 |
| sao | 0 | 271.9 KiB, 93 | 290.8 KiB, 15 | 269.2 KiB, 38 | 2063.6 KiB, 669 | 290.1 KiB, 3 |
| sao | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 15289.4 KiB, 101 | 8226.1 KiB, 3 |
| sao | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 15289.4 KiB, 101 | 65574.1 KiB, 3 |
| webster | 0 | 271.9 KiB, 522 | 290.8 KiB, 48 | 270.1 KiB, 206 | 2064.5 KiB, 3807 | 290.1 KiB, 3 |
| webster | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57363.4 KiB, 202 | 8226.1 KiB, 3 |
| webster | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 106043.4 KiB, 104 | 65574.1 KiB, 3 |
| xml | 0 | 271.9 KiB, 79 | 290.8 KiB, 14 | 269.2 KiB, 32 | 2063.5 KiB, 571 | 290.1 KiB, 3 |
| xml | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 13427.4 KiB, 101 | 8226.1 KiB, 3 |
| xml | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 13427.4 KiB, 101 | 65574.1 KiB, 3 |
| x-ray | 0 | 271.9 KiB, 118 | 290.8 KiB, 17 | 269.2 KiB, 48 | 2063.6 KiB, 857 | 290.1 KiB, 3 |
| x-ray | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 24675.0 KiB, 102 | 8226.1 KiB, 3 |
| x-ray | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 24675.0 KiB, 102 | 65574.1 KiB, 3 |

#### Large files

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 0 | 271.9 KiB, 1250 | 290.8 KiB, 104 | 271.1 KiB, 487 | 2065.4 KiB, 9127 | 290.1 KiB, 3 |
| enwik8 | 6 | 8207.9 KiB, 78 | 8226.8 KiB, 12 | 8205.1 KiB, 42 | 57363.4 KiB, 403 | 8226.1 KiB, 3 |
| enwik8 | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 228751.3 KiB, 105 | 65574.1 KiB, 3 |
| linux-6.12.tar | 0 | 271.9 KiB, 1250 | 290.8 KiB, 104 | 271.1 KiB, 487 | 2065.4 KiB, 9127 | 290.1 KiB, 3 |
| linux-6.12.tar | 6 | 8207.9 KiB, 78 | 8226.8 KiB, 12 | 8205.1 KiB, 42 | 57359.4 KiB, 403 | 8226.1 KiB, 3 |
| linux-6.12.tar | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 228751.3 KiB, 105 | 65574.1 KiB, 3 |

#### Canterbury

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 419.9 KiB, 96 | 290.1 KiB, 3 |
| alice29.txt | 6 | 271.9 KiB, 16 | 8226.8 KiB, 9 | 269.1 KiB, 7 | 420.0 KiB, 96 | 8226.1 KiB, 3 |
| alice29.txt | 9e | 271.9 KiB, 16 | 65574.2 KiB, 9 | 269.1 KiB, 7 | 420.0 KiB, 96 | 65574.1 KiB, 3 |
| asyoulik.txt | 0 | 143.9 KiB, 14 | 290.8 KiB, 9 | 141.1 KiB, 6 | 265.7 KiB, 95 | 290.1 KiB, 3 |
| asyoulik.txt | 6 | 143.9 KiB, 15 | 8226.8 KiB, 9 | 141.1 KiB, 6 | 265.7 KiB, 95 | 8226.1 KiB, 3 |
| asyoulik.txt | 9e | 143.9 KiB, 15 | 65574.2 KiB, 9 | 141.1 KiB, 6 | 265.7 KiB, 95 | 65574.1 KiB, 3 |
| cp.html | 0 | 47.9 KiB, 12 | 290.8 KiB, 9 | 77.1 KiB, 5 | 71.4 KiB, 93 | 290.1 KiB, 3 |
| cp.html | 6 | 47.9 KiB, 13 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 71.5 KiB, 93 | 8226.1 KiB, 3 |
| cp.html | 9e | 47.9 KiB, 13 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 71.4 KiB, 93 | 65574.1 KiB, 3 |
| fields.c | 0 | 31.9 KiB, 11 | 290.8 KiB, 9 | 77.1 KiB, 5 | 42.3 KiB, 92 | 290.1 KiB, 3 |
| fields.c | 6 | 31.9 KiB, 12 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 42.3 KiB, 92 | 8226.1 KiB, 3 |
| fields.c | 9e | 31.9 KiB, 12 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 42.3 KiB, 92 | 65574.1 KiB, 3 |
| grammar.lsp | 0 | 19.9 KiB, 9 | 290.8 KiB, 9 | 77.1 KiB, 5 | 23.0 KiB, 90 | 290.1 KiB, 3 |
| grammar.lsp | 6 | 19.9 KiB, 10 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 23.0 KiB, 90 | 8226.1 KiB, 3 |
| grammar.lsp | 9e | 19.9 KiB, 10 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 23.0 KiB, 90 | 65574.1 KiB, 3 |
| kennedy.xls | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 2045.0 KiB, 98 | 290.1 KiB, 3 |
| kennedy.xls | 6 | 1039.9 KiB, 18 | 8226.8 KiB, 9 | 1037.1 KiB, 9 | 2045.0 KiB, 98 | 8226.1 KiB, 3 |
| kennedy.xls | 9e | 1039.9 KiB, 18 | 65574.2 KiB, 9 | 1037.1 KiB, 9 | 2045.0 KiB, 98 | 65574.1 KiB, 3 |
| lcet10.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 944.1 KiB, 97 | 290.1 KiB, 3 |
| lcet10.txt | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 944.1 KiB, 97 | 8226.1 KiB, 3 |
| lcet10.txt | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 944.2 KiB, 97 | 65574.1 KiB, 3 |
| plrabn12.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 998.0 KiB, 97 | 290.1 KiB, 3 |
| plrabn12.txt | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 998.0 KiB, 97 | 8226.1 KiB, 3 |
| plrabn12.txt | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 997.9 KiB, 97 | 65574.1 KiB, 3 |
| ptt5 | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1028.6 KiB, 97 | 290.1 KiB, 3 |
| ptt5 | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 1028.6 KiB, 97 | 8226.1 KiB, 3 |
| ptt5 | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 1028.6 KiB, 97 | 65574.1 KiB, 3 |
| sum | 0 | 79.9 KiB, 13 | 290.8 KiB, 9 | 77.1 KiB, 5 | 116.7 KiB, 94 | 290.1 KiB, 3 |
| sum | 6 | 79.9 KiB, 14 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 116.7 KiB, 94 | 8226.1 KiB, 3 |
| sum | 9e | 79.9 KiB, 14 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 116.7 KiB, 94 | 65574.1 KiB, 3 |
| xargs.1 | 0 | 23.9 KiB, 10 | 290.8 KiB, 9 | 77.1 KiB, 5 | 27.5 KiB, 91 | 290.1 KiB, 3 |
| xargs.1 | 6 | 23.9 KiB, 11 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 27.5 KiB, 91 | 8226.1 KiB, 3 |
| xargs.1 | 9e | 23.9 KiB, 11 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 27.5 KiB, 91 | 65574.1 KiB, 3 |
| E.coli | 0 | 271.9 KiB, 67 | 290.8 KiB, 13 | 269.2 KiB, 28 | 2063.5 KiB, 478 | 290.1 KiB, 3 |
| E.coli | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 12737.4 KiB, 101 | 8226.1 KiB, 3 |
| E.coli | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 12737.3 KiB, 101 | 65574.1 KiB, 3 |
| bible.txt | 0 | 271.9 KiB, 54 | 290.8 KiB, 12 | 269.1 KiB, 22 | 2063.4 KiB, 383 | 290.1 KiB, 3 |
| bible.txt | 6 | 4111.9 KiB, 20 | 8226.8 KiB, 9 | 4109.1 KiB, 11 | 8063.9 KiB, 100 | 8226.1 KiB, 3 |
| bible.txt | 9e | 4111.9 KiB, 20 | 65574.2 KiB, 9 | 4109.1 KiB, 11 | 8063.9 KiB, 100 | 65574.1 KiB, 3 |
| world192.txt | 0 | 271.9 KiB, 41 | 290.8 KiB, 11 | 269.1 KiB, 17 | 2063.4 KiB, 287 | 290.1 KiB, 3 |
| world192.txt | 6 | 4111.9 KiB, 20 | 8226.8 KiB, 9 | 4109.1 KiB, 11 | 6526.8 KiB, 100 | 8226.1 KiB, 3 |
| world192.txt | 9e | 4111.9 KiB, 20 | 65574.2 KiB, 9 | 4109.1 KiB, 11 | 6526.7 KiB, 100 | 65574.1 KiB, 3 |

#### Rust's executables

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rustc | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1670.5 KiB, 98 | 290.1 KiB, 3 |
| rustc | 6 | 1039.9 KiB, 18 | 8226.8 KiB, 9 | 1037.1 KiB, 9 | 1670.5 KiB, 98 | 8226.1 KiB, 3 |
| rustc | 9e | 1039.9 KiB, 18 | 65574.2 KiB, 9 | 1037.1 KiB, 9 | 1670.5 KiB, 98 | 65574.1 KiB, 3 |
| librustc_driver.so | 0 | 271.9 KiB, 1965 | 290.8 KiB, 159 | 273.1 KiB, 763 | 2067.4 KiB, 14355 | 290.1 KiB, 3 |
| librustc_driver.so | 6 | 8207.9 KiB, 135 | 8226.8 KiB, 15 | 8205.2 KiB, 73 | 57363.5 KiB, 703 | 8226.1 KiB, 3 |
| librustc_driver.so | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 416343.3 KiB, 107 | 65574.1 KiB, 3 |
| cargo | 0 | 271.9 KiB, 535 | 290.8 KiB, 49 | 270.1 KiB, 211 | 2064.4 KiB, 3904 | 290.1 KiB, 3 |
| cargo | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57363.3 KiB, 205 | 8226.1 KiB, 3 |
| cargo | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 107399.3 KiB, 106 | 65574.1 KiB, 3 |

Ours relative to liblzma's peak: 0.94x at the median, and at most 1.00x.

### AMD EPYC 9R45 (1 vCPU under EC2 Nitro), in pieces of 4 KiB

Measured 2026-11-21, at `b66411c`, `decode-xz` 0.0.4.

The size of each decoder's value: ours 336 B, liblzma 136 B, lzma-rust2
4440 B, lzma-rs 0 B, xz4rust 8 B.

#### Synthetic

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| random | 0 | 261.4 KiB, 98 | 290.8 KiB, 16 | 257.2 KiB, 35 | 2925.7 KiB, 805 | 290.1 KiB, 3 |
| random | 6 | 8197.4 KiB, 20 | 8226.8 KiB, 9 | 8193.1 KiB, 11 | 23349.0 KiB, 228 | 8226.1 KiB, 3 |
| random | 9e | 16393.4 KiB, 21 | 65574.2 KiB, 9 | 8193.1 KiB, 11 | 23347.5 KiB, 228 | 65574.1 KiB, 3 |
| zeros | 0 | 271.9 KiB, 106 | 290.8 KiB, 16 | 269.2 KiB, 43 | 2067.6 KiB, 765 | 290.1 KiB, 3 |
| zeros | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 16403.5 KiB, 102 | 8226.1 KiB, 3 |
| zeros | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 16403.4 KiB, 102 | 65574.1 KiB, 3 |
| pattern | 0 | 271.9 KiB, 106 | 290.8 KiB, 16 | 269.2 KiB, 43 | 2067.6 KiB, 765 | 290.1 KiB, 3 |
| pattern | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 16403.4 KiB, 102 | 8226.1 KiB, 3 |
| pattern | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 16403.4 KiB, 102 | 65574.1 KiB, 3 |
| far | 0 | 261.4 KiB, 98 | 290.8 KiB, 16 | 257.2 KiB, 35 | 2925.6 KiB, 805 | 290.1 KiB, 3 |
| far | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 23357.9 KiB, 159 | 8226.1 KiB, 3 |
| far | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 23357.9 KiB, 159 | 65574.1 KiB, 3 |

#### Silesia

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| dickens | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2067.5 KiB, 956 | 290.1 KiB, 3 |
| dickens | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26357.0 KiB, 103 | 8226.1 KiB, 3 |
| dickens | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26357.0 KiB, 103 | 65574.1 KiB, 3 |
| mozilla | 0 | 271.9 KiB, 639 | 290.8 KiB, 57 | 270.1 KiB, 251 | 2949.7 KiB, 4654 | 290.1 KiB, 3 |
| mozilla | 6 | 8207.9 KiB, 56 | 8226.8 KiB, 11 | 8205.1 KiB, 29 | 57367.5 KiB, 300 | 8226.1 KiB, 3 |
| mozilla | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 115583.4 KiB, 106 | 65574.1 KiB, 3 |
| mr | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2067.5 KiB, 956 | 290.1 KiB, 3 |
| mr | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26140.3 KiB, 103 | 8226.1 KiB, 3 |
| mr | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26140.3 KiB, 103 | 65574.1 KiB, 3 |
| nci | 0 | 271.9 KiB, 418 | 290.8 KiB, 40 | 269.6 KiB, 165 | 2067.9 KiB, 3047 | 290.1 KiB, 3 |
| nci | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57367.4 KiB, 202 | 8226.1 KiB, 3 |
| nci | 9e | 32787.9 KiB, 23 | 65574.2 KiB, 9 | 32785.1 KiB, 14 | 65558.4 KiB, 104 | 65574.1 KiB, 3 |
| ooffice | 0 | 271.9 KiB, 80 | 290.8 KiB, 14 | 269.2 KiB, 33 | 2067.5 KiB, 575 | 290.1 KiB, 3 |
| ooffice | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 14219.4 KiB, 102 | 8226.1 KiB, 3 |
| ooffice | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 14219.4 KiB, 102 | 65574.1 KiB, 3 |
| osdb | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2067.5 KiB, 956 | 290.1 KiB, 3 |
| osdb | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26252.7 KiB, 103 | 8226.1 KiB, 3 |
| osdb | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26252.7 KiB, 103 | 65574.1 KiB, 3 |
| reymont | 0 | 271.9 KiB, 93 | 290.8 KiB, 15 | 269.2 KiB, 38 | 2067.6 KiB, 669 | 290.1 KiB, 3 |
| reymont | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 14683.3 KiB, 102 | 8226.1 KiB, 3 |
| reymont | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 14683.3 KiB, 102 | 65574.1 KiB, 3 |
| samba | 0 | 271.9 KiB, 275 | 290.8 KiB, 29 | 269.6 KiB, 110 | 2068.0 KiB, 2002 | 290.1 KiB, 3 |
| samba | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 53891.4 KiB, 104 | 8226.1 KiB, 3 |
| samba | 9e | 32787.9 KiB, 23 | 65574.2 KiB, 9 | 32785.1 KiB, 14 | 53891.4 KiB, 104 | 65574.1 KiB, 3 |
| sao | 0 | 271.9 KiB, 93 | 290.8 KiB, 15 | 269.2 KiB, 38 | 2067.5 KiB, 670 | 290.1 KiB, 3 |
| sao | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 15293.4 KiB, 102 | 8226.1 KiB, 3 |
| sao | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 15293.4 KiB, 102 | 65574.1 KiB, 3 |
| webster | 0 | 271.9 KiB, 522 | 290.8 KiB, 48 | 270.1 KiB, 206 | 2068.5 KiB, 3808 | 290.1 KiB, 3 |
| webster | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57367.4 KiB, 203 | 8226.1 KiB, 3 |
| webster | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 106047.3 KiB, 105 | 65574.1 KiB, 3 |
| xml | 0 | 271.9 KiB, 79 | 290.8 KiB, 14 | 269.2 KiB, 32 | 2067.5 KiB, 572 | 290.1 KiB, 3 |
| xml | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 13431.4 KiB, 102 | 8226.1 KiB, 3 |
| xml | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 13431.4 KiB, 102 | 65574.1 KiB, 3 |
| x-ray | 0 | 271.9 KiB, 118 | 290.8 KiB, 17 | 269.2 KiB, 48 | 2067.6 KiB, 858 | 290.1 KiB, 3 |
| x-ray | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 24679.0 KiB, 103 | 8226.1 KiB, 3 |
| x-ray | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 24679.0 KiB, 103 | 65574.1 KiB, 3 |

#### Large files

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 0 | 271.9 KiB, 1250 | 290.8 KiB, 104 | 271.1 KiB, 487 | 2069.4 KiB, 9128 | 290.1 KiB, 3 |
| enwik8 | 6 | 8207.9 KiB, 78 | 8226.8 KiB, 12 | 8205.1 KiB, 42 | 57367.4 KiB, 404 | 8226.1 KiB, 3 |
| enwik8 | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 228755.4 KiB, 106 | 65574.1 KiB, 3 |
| linux-6.12.tar | 0 | 271.9 KiB, 1250 | 290.8 KiB, 104 | 271.1 KiB, 487 | 2069.5 KiB, 9128 | 290.1 KiB, 3 |
| linux-6.12.tar | 6 | 8207.9 KiB, 78 | 8226.8 KiB, 12 | 8205.1 KiB, 42 | 57363.5 KiB, 404 | 8226.1 KiB, 3 |
| linux-6.12.tar | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 228755.4 KiB, 106 | 65574.1 KiB, 3 |

#### Canterbury

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 424.0 KiB, 97 | 290.1 KiB, 3 |
| alice29.txt | 6 | 271.9 KiB, 16 | 8226.8 KiB, 9 | 269.1 KiB, 7 | 424.0 KiB, 97 | 8226.1 KiB, 3 |
| alice29.txt | 9e | 271.9 KiB, 16 | 65574.2 KiB, 9 | 269.1 KiB, 7 | 423.9 KiB, 97 | 65574.1 KiB, 3 |
| asyoulik.txt | 0 | 143.9 KiB, 14 | 290.8 KiB, 9 | 141.1 KiB, 6 | 269.7 KiB, 96 | 290.1 KiB, 3 |
| asyoulik.txt | 6 | 143.9 KiB, 15 | 8226.8 KiB, 9 | 141.1 KiB, 6 | 269.7 KiB, 96 | 8226.1 KiB, 3 |
| asyoulik.txt | 9e | 143.9 KiB, 15 | 65574.2 KiB, 9 | 141.1 KiB, 6 | 269.7 KiB, 96 | 65574.1 KiB, 3 |
| cp.html | 0 | 47.9 KiB, 12 | 290.8 KiB, 9 | 77.1 KiB, 5 | 75.5 KiB, 94 | 290.1 KiB, 3 |
| cp.html | 6 | 47.9 KiB, 13 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 75.5 KiB, 94 | 8226.1 KiB, 3 |
| cp.html | 9e | 47.9 KiB, 13 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 75.4 KiB, 94 | 65574.1 KiB, 3 |
| fields.c | 0 | 31.9 KiB, 11 | 290.8 KiB, 9 | 77.1 KiB, 5 | 46.3 KiB, 93 | 290.1 KiB, 3 |
| fields.c | 6 | 31.9 KiB, 12 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 46.3 KiB, 93 | 8226.1 KiB, 3 |
| fields.c | 9e | 31.9 KiB, 12 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 46.3 KiB, 93 | 65574.1 KiB, 3 |
| grammar.lsp | 0 | 19.9 KiB, 9 | 290.8 KiB, 9 | 77.1 KiB, 5 | 27.0 KiB, 91 | 290.1 KiB, 3 |
| grammar.lsp | 6 | 19.9 KiB, 10 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 27.1 KiB, 91 | 8226.1 KiB, 3 |
| grammar.lsp | 9e | 19.9 KiB, 10 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 27.1 KiB, 91 | 65574.1 KiB, 3 |
| kennedy.xls | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 2049.0 KiB, 99 | 290.1 KiB, 3 |
| kennedy.xls | 6 | 1039.9 KiB, 18 | 8226.8 KiB, 9 | 1037.1 KiB, 9 | 2049.0 KiB, 99 | 8226.1 KiB, 3 |
| kennedy.xls | 9e | 1039.9 KiB, 18 | 65574.2 KiB, 9 | 1037.1 KiB, 9 | 2049.0 KiB, 99 | 65574.1 KiB, 3 |
| lcet10.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 948.1 KiB, 98 | 290.1 KiB, 3 |
| lcet10.txt | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 948.1 KiB, 98 | 8226.1 KiB, 3 |
| lcet10.txt | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 948.1 KiB, 98 | 65574.1 KiB, 3 |
| plrabn12.txt | 0 | 272.0 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1002.0 KiB, 98 | 290.1 KiB, 3 |
| plrabn12.txt | 6 | 528.0 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 1002.0 KiB, 98 | 8226.1 KiB, 3 |
| plrabn12.txt | 9e | 528.0 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 1002.0 KiB, 98 | 65574.1 KiB, 3 |
| ptt5 | 0 | 272.0 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1032.6 KiB, 98 | 290.1 KiB, 3 |
| ptt5 | 6 | 528.0 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 1032.6 KiB, 98 | 8226.1 KiB, 3 |
| ptt5 | 9e | 528.0 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 1032.6 KiB, 98 | 65574.1 KiB, 3 |
| sum | 0 | 80.0 KiB, 13 | 290.8 KiB, 9 | 77.1 KiB, 5 | 120.8 KiB, 95 | 290.1 KiB, 3 |
| sum | 6 | 80.0 KiB, 14 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 120.7 KiB, 95 | 8226.1 KiB, 3 |
| sum | 9e | 79.9 KiB, 14 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 120.7 KiB, 95 | 65574.1 KiB, 3 |
| xargs.1 | 0 | 23.9 KiB, 10 | 290.8 KiB, 9 | 77.1 KiB, 5 | 31.5 KiB, 92 | 290.1 KiB, 3 |
| xargs.1 | 6 | 23.9 KiB, 11 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 31.5 KiB, 92 | 8226.1 KiB, 3 |
| xargs.1 | 9e | 23.9 KiB, 11 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 31.5 KiB, 92 | 65574.1 KiB, 3 |
| E.coli | 0 | 271.9 KiB, 67 | 290.8 KiB, 13 | 269.2 KiB, 28 | 2067.5 KiB, 479 | 290.1 KiB, 3 |
| E.coli | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 12741.4 KiB, 102 | 8226.1 KiB, 3 |
| E.coli | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 12741.4 KiB, 102 | 65574.1 KiB, 3 |
| bible.txt | 0 | 271.9 KiB, 54 | 290.8 KiB, 12 | 269.1 KiB, 22 | 2067.5 KiB, 384 | 290.1 KiB, 3 |
| bible.txt | 6 | 4111.9 KiB, 20 | 8226.8 KiB, 9 | 4109.1 KiB, 11 | 8067.9 KiB, 101 | 8226.1 KiB, 3 |
| bible.txt | 9e | 4111.9 KiB, 20 | 65574.2 KiB, 9 | 4109.1 KiB, 11 | 8067.9 KiB, 101 | 65574.1 KiB, 3 |
| world192.txt | 0 | 271.9 KiB, 41 | 290.8 KiB, 11 | 269.1 KiB, 17 | 2067.5 KiB, 288 | 290.1 KiB, 3 |
| world192.txt | 6 | 4111.9 KiB, 20 | 8226.8 KiB, 9 | 4109.1 KiB, 11 | 6530.9 KiB, 101 | 8226.1 KiB, 3 |
| world192.txt | 9e | 4111.9 KiB, 20 | 65574.2 KiB, 9 | 4109.1 KiB, 11 | 6530.9 KiB, 101 | 65574.1 KiB, 3 |

#### Rust's executables

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rustc | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1674.6 KiB, 99 | 290.1 KiB, 3 |
| rustc | 6 | 1039.9 KiB, 18 | 8226.8 KiB, 9 | 1037.1 KiB, 9 | 1674.6 KiB, 99 | 8226.1 KiB, 3 |
| rustc | 9e | 1039.9 KiB, 18 | 65574.2 KiB, 9 | 1037.1 KiB, 9 | 1674.6 KiB, 99 | 65574.1 KiB, 3 |
| librustc_driver.so | 0 | 271.9 KiB, 1965 | 290.8 KiB, 159 | 273.1 KiB, 763 | 2071.5 KiB, 14356 | 290.1 KiB, 3 |
| librustc_driver.so | 6 | 8207.9 KiB, 135 | 8226.8 KiB, 15 | 8205.2 KiB, 73 | 57367.5 KiB, 704 | 8226.1 KiB, 3 |
| librustc_driver.so | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 416347.3 KiB, 108 | 65574.1 KiB, 3 |
| cargo | 0 | 271.9 KiB, 535 | 290.8 KiB, 49 | 270.1 KiB, 211 | 2068.5 KiB, 3905 | 290.1 KiB, 3 |
| cargo | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57367.4 KiB, 206 | 8226.1 KiB, 3 |
| cargo | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 107403.4 KiB, 107 | 65574.1 KiB, 3 |

Ours relative to liblzma's peak: 0.94x at the median, and at most 1.00x.

### AMD EPYC 9R45 (1 vCPU under EC2 Nitro), in pieces of 64 KiB

Measured 2026-11-21, at `b66411c`, `decode-xz` 0.0.4.

The size of each decoder's value: ours 336 B, liblzma 136 B, lzma-rust2
4440 B, lzma-rs 0 B, xz4rust 8 B.

#### Synthetic

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| random | 0 | 261.4 KiB, 98 | 290.8 KiB, 16 | 257.2 KiB, 35 | 2985.7 KiB, 805 | 290.1 KiB, 3 |
| random | 6 | 8197.4 KiB, 20 | 8226.8 KiB, 9 | 8193.1 KiB, 11 | 23409.0 KiB, 228 | 8226.1 KiB, 3 |
| random | 9e | 16393.4 KiB, 21 | 65574.2 KiB, 9 | 8193.1 KiB, 11 | 23407.5 KiB, 228 | 65574.1 KiB, 3 |
| zeros | 0 | 271.9 KiB, 106 | 290.8 KiB, 16 | 269.2 KiB, 43 | 2127.6 KiB, 765 | 290.1 KiB, 3 |
| zeros | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 16463.4 KiB, 102 | 8226.1 KiB, 3 |
| zeros | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 16463.4 KiB, 102 | 65574.1 KiB, 3 |
| pattern | 0 | 271.9 KiB, 106 | 290.8 KiB, 16 | 269.2 KiB, 43 | 2127.6 KiB, 765 | 290.1 KiB, 3 |
| pattern | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 16463.4 KiB, 102 | 8226.1 KiB, 3 |
| pattern | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 16463.4 KiB, 102 | 65574.1 KiB, 3 |
| far | 0 | 261.4 KiB, 98 | 290.8 KiB, 16 | 257.2 KiB, 35 | 2985.6 KiB, 805 | 290.1 KiB, 3 |
| far | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 23417.9 KiB, 159 | 8226.1 KiB, 3 |
| far | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 23417.9 KiB, 159 | 65574.1 KiB, 3 |

#### Silesia

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| dickens | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2127.6 KiB, 956 | 290.1 KiB, 3 |
| dickens | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26417.0 KiB, 103 | 8226.1 KiB, 3 |
| dickens | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26416.9 KiB, 103 | 65574.1 KiB, 3 |
| mozilla | 0 | 271.9 KiB, 639 | 290.8 KiB, 57 | 270.1 KiB, 251 | 3009.7 KiB, 4654 | 290.1 KiB, 3 |
| mozilla | 6 | 8207.9 KiB, 56 | 8226.8 KiB, 11 | 8205.1 KiB, 29 | 57427.5 KiB, 300 | 8226.1 KiB, 3 |
| mozilla | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 115643.4 KiB, 106 | 65574.1 KiB, 3 |
| mr | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2127.6 KiB, 956 | 290.1 KiB, 3 |
| mr | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26200.3 KiB, 103 | 8226.1 KiB, 3 |
| mr | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26200.3 KiB, 103 | 65574.1 KiB, 3 |
| nci | 0 | 271.9 KiB, 418 | 290.8 KiB, 40 | 269.6 KiB, 165 | 2128.0 KiB, 3047 | 290.1 KiB, 3 |
| nci | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57427.4 KiB, 202 | 8226.1 KiB, 3 |
| nci | 9e | 32787.9 KiB, 23 | 65574.2 KiB, 9 | 32785.1 KiB, 14 | 65618.5 KiB, 104 | 65574.1 KiB, 3 |
| ooffice | 0 | 271.9 KiB, 80 | 290.8 KiB, 14 | 269.2 KiB, 33 | 2127.5 KiB, 575 | 290.1 KiB, 3 |
| ooffice | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 14279.4 KiB, 102 | 8226.1 KiB, 3 |
| ooffice | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 14279.4 KiB, 102 | 65574.1 KiB, 3 |
| osdb | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2127.6 KiB, 956 | 290.1 KiB, 3 |
| osdb | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26312.7 KiB, 103 | 8226.1 KiB, 3 |
| osdb | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26312.8 KiB, 103 | 65574.1 KiB, 3 |
| reymont | 0 | 271.9 KiB, 93 | 290.8 KiB, 15 | 269.2 KiB, 38 | 2127.6 KiB, 669 | 290.1 KiB, 3 |
| reymont | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 14743.4 KiB, 102 | 8226.1 KiB, 3 |
| reymont | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 14743.3 KiB, 102 | 65574.1 KiB, 3 |
| samba | 0 | 271.9 KiB, 275 | 290.8 KiB, 29 | 269.6 KiB, 110 | 2128.0 KiB, 2002 | 290.1 KiB, 3 |
| samba | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 53951.4 KiB, 104 | 8226.1 KiB, 3 |
| samba | 9e | 32787.9 KiB, 23 | 65574.2 KiB, 9 | 32785.1 KiB, 14 | 53951.4 KiB, 104 | 65574.1 KiB, 3 |
| sao | 0 | 271.9 KiB, 93 | 290.8 KiB, 15 | 269.2 KiB, 38 | 2127.6 KiB, 670 | 290.1 KiB, 3 |
| sao | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 15353.4 KiB, 102 | 8226.1 KiB, 3 |
| sao | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 15353.4 KiB, 102 | 65574.1 KiB, 3 |
| webster | 0 | 271.9 KiB, 522 | 290.8 KiB, 48 | 270.1 KiB, 206 | 2128.5 KiB, 3808 | 290.1 KiB, 3 |
| webster | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57427.4 KiB, 203 | 8226.1 KiB, 3 |
| webster | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 106107.4 KiB, 105 | 65574.1 KiB, 3 |
| xml | 0 | 271.9 KiB, 79 | 290.8 KiB, 14 | 269.2 KiB, 32 | 2127.5 KiB, 572 | 290.1 KiB, 3 |
| xml | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 13491.4 KiB, 102 | 8226.1 KiB, 3 |
| xml | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 13491.4 KiB, 102 | 65574.1 KiB, 3 |
| x-ray | 0 | 271.9 KiB, 118 | 290.8 KiB, 17 | 269.2 KiB, 48 | 2127.6 KiB, 858 | 290.1 KiB, 3 |
| x-ray | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 24739.1 KiB, 103 | 8226.1 KiB, 3 |
| x-ray | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 24739.1 KiB, 103 | 65574.1 KiB, 3 |

#### Large files

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 0 | 271.9 KiB, 1250 | 290.8 KiB, 104 | 271.1 KiB, 487 | 2129.4 KiB, 9128 | 290.1 KiB, 3 |
| enwik8 | 6 | 8207.9 KiB, 78 | 8226.8 KiB, 12 | 8205.1 KiB, 42 | 57427.4 KiB, 404 | 8226.1 KiB, 3 |
| enwik8 | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 228815.3 KiB, 106 | 65574.1 KiB, 3 |
| linux-6.12.tar | 0 | 271.9 KiB, 1250 | 290.8 KiB, 104 | 271.1 KiB, 487 | 2129.4 KiB, 9128 | 290.1 KiB, 3 |
| linux-6.12.tar | 6 | 8207.9 KiB, 78 | 8226.8 KiB, 12 | 8205.1 KiB, 42 | 57423.4 KiB, 404 | 8226.1 KiB, 3 |
| linux-6.12.tar | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 228815.3 KiB, 106 | 65574.1 KiB, 3 |

#### Canterbury

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 483.9 KiB, 97 | 290.1 KiB, 3 |
| alice29.txt | 6 | 271.9 KiB, 16 | 8226.8 KiB, 9 | 269.1 KiB, 7 | 484.0 KiB, 97 | 8226.1 KiB, 3 |
| alice29.txt | 9e | 271.9 KiB, 16 | 65574.2 KiB, 9 | 269.1 KiB, 7 | 484.0 KiB, 97 | 65574.1 KiB, 3 |
| asyoulik.txt | 0 | 143.9 KiB, 14 | 290.8 KiB, 9 | 141.1 KiB, 6 | 329.7 KiB, 96 | 290.1 KiB, 3 |
| asyoulik.txt | 6 | 143.9 KiB, 15 | 8226.8 KiB, 9 | 141.1 KiB, 6 | 329.7 KiB, 96 | 8226.1 KiB, 3 |
| asyoulik.txt | 9e | 143.9 KiB, 15 | 65574.2 KiB, 9 | 141.1 KiB, 6 | 329.7 KiB, 96 | 65574.1 KiB, 3 |
| cp.html | 0 | 47.9 KiB, 12 | 290.8 KiB, 9 | 77.1 KiB, 5 | 135.4 KiB, 94 | 290.1 KiB, 3 |
| cp.html | 6 | 47.9 KiB, 13 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 135.5 KiB, 94 | 8226.1 KiB, 3 |
| cp.html | 9e | 47.9 KiB, 13 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 135.4 KiB, 94 | 65574.1 KiB, 3 |
| fields.c | 0 | 31.9 KiB, 11 | 290.8 KiB, 9 | 77.1 KiB, 5 | 106.3 KiB, 93 | 290.1 KiB, 3 |
| fields.c | 6 | 31.9 KiB, 12 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 106.3 KiB, 93 | 8226.1 KiB, 3 |
| fields.c | 9e | 31.9 KiB, 12 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 106.3 KiB, 93 | 65574.1 KiB, 3 |
| grammar.lsp | 0 | 19.9 KiB, 9 | 290.8 KiB, 9 | 77.1 KiB, 5 | 87.0 KiB, 91 | 290.1 KiB, 3 |
| grammar.lsp | 6 | 19.9 KiB, 10 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 87.0 KiB, 91 | 8226.1 KiB, 3 |
| grammar.lsp | 9e | 19.9 KiB, 10 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 87.0 KiB, 91 | 65574.1 KiB, 3 |
| kennedy.xls | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 2109.0 KiB, 99 | 290.1 KiB, 3 |
| kennedy.xls | 6 | 1039.9 KiB, 18 | 8226.8 KiB, 9 | 1037.1 KiB, 9 | 2109.0 KiB, 99 | 8226.1 KiB, 3 |
| kennedy.xls | 9e | 1039.9 KiB, 18 | 65574.2 KiB, 9 | 1037.1 KiB, 9 | 2109.0 KiB, 99 | 65574.1 KiB, 3 |
| lcet10.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1008.1 KiB, 98 | 290.1 KiB, 3 |
| lcet10.txt | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 1008.1 KiB, 98 | 8226.1 KiB, 3 |
| lcet10.txt | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 1008.2 KiB, 98 | 65574.1 KiB, 3 |
| plrabn12.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1062.0 KiB, 98 | 290.1 KiB, 3 |
| plrabn12.txt | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 1062.0 KiB, 98 | 8226.1 KiB, 3 |
| plrabn12.txt | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 1061.9 KiB, 98 | 65574.1 KiB, 3 |
| ptt5 | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1092.6 KiB, 98 | 290.1 KiB, 3 |
| ptt5 | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 1092.6 KiB, 98 | 8226.1 KiB, 3 |
| ptt5 | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 1092.6 KiB, 98 | 65574.1 KiB, 3 |
| sum | 0 | 79.9 KiB, 13 | 290.8 KiB, 9 | 77.1 KiB, 5 | 180.7 KiB, 95 | 290.1 KiB, 3 |
| sum | 6 | 79.9 KiB, 14 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 180.7 KiB, 95 | 8226.1 KiB, 3 |
| sum | 9e | 79.9 KiB, 14 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 180.7 KiB, 95 | 65574.1 KiB, 3 |
| xargs.1 | 0 | 23.9 KiB, 10 | 290.8 KiB, 9 | 77.1 KiB, 5 | 91.5 KiB, 92 | 290.1 KiB, 3 |
| xargs.1 | 6 | 23.9 KiB, 11 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 91.5 KiB, 92 | 8226.1 KiB, 3 |
| xargs.1 | 9e | 23.9 KiB, 11 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 91.5 KiB, 92 | 65574.1 KiB, 3 |
| E.coli | 0 | 271.9 KiB, 67 | 290.8 KiB, 13 | 269.2 KiB, 28 | 2127.5 KiB, 479 | 290.1 KiB, 3 |
| E.coli | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 12801.4 KiB, 102 | 8226.1 KiB, 3 |
| E.coli | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 12801.3 KiB, 102 | 65574.1 KiB, 3 |
| bible.txt | 0 | 271.9 KiB, 54 | 290.8 KiB, 12 | 269.1 KiB, 22 | 2127.4 KiB, 384 | 290.1 KiB, 3 |
| bible.txt | 6 | 4111.9 KiB, 20 | 8226.8 KiB, 9 | 4109.1 KiB, 11 | 8127.9 KiB, 101 | 8226.1 KiB, 3 |
| bible.txt | 9e | 4111.9 KiB, 20 | 65574.2 KiB, 9 | 4109.1 KiB, 11 | 8127.9 KiB, 101 | 65574.1 KiB, 3 |
| world192.txt | 0 | 271.9 KiB, 41 | 290.8 KiB, 11 | 269.1 KiB, 17 | 2127.4 KiB, 288 | 290.1 KiB, 3 |
| world192.txt | 6 | 4111.9 KiB, 20 | 8226.8 KiB, 9 | 4109.1 KiB, 11 | 6590.8 KiB, 101 | 8226.1 KiB, 3 |
| world192.txt | 9e | 4111.9 KiB, 20 | 65574.2 KiB, 9 | 4109.1 KiB, 11 | 6590.7 KiB, 101 | 65574.1 KiB, 3 |

#### Rust's executables

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rustc | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1734.6 KiB, 99 | 290.1 KiB, 3 |
| rustc | 6 | 1039.9 KiB, 18 | 8226.8 KiB, 9 | 1037.1 KiB, 9 | 1734.6 KiB, 99 | 8226.1 KiB, 3 |
| rustc | 9e | 1039.9 KiB, 18 | 65574.2 KiB, 9 | 1037.1 KiB, 9 | 1734.6 KiB, 99 | 65574.1 KiB, 3 |
| librustc_driver.so | 0 | 271.9 KiB, 1965 | 290.8 KiB, 159 | 273.1 KiB, 763 | 2131.4 KiB, 14356 | 290.1 KiB, 3 |
| librustc_driver.so | 6 | 8207.9 KiB, 135 | 8226.8 KiB, 15 | 8205.2 KiB, 73 | 57427.5 KiB, 704 | 8226.1 KiB, 3 |
| librustc_driver.so | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 416407.3 KiB, 108 | 65574.1 KiB, 3 |
| cargo | 0 | 271.9 KiB, 535 | 290.8 KiB, 49 | 270.1 KiB, 211 | 2128.4 KiB, 3905 | 290.1 KiB, 3 |
| cargo | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57427.4 KiB, 206 | 8226.1 KiB, 3 |
| cargo | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 107463.3 KiB, 107 | 65574.1 KiB, 3 |

Ours relative to liblzma's peak: 0.94x at the median, and at most 1.00x.

### ARM Neoverse-V3 (1 vCPU under EC2 Nitro), whole

Measured 2026-11-21, at `b66411c`, `decode-xz` 0.0.4.

The size of each decoder's value: ours 336 B, liblzma 136 B, lzma-rust2
4440 B, lzma-rs 0 B, xz4rust 8 B.

#### Synthetic

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| random | 0 | 261.4 KiB, 98 | 290.8 KiB, 16 | 257.2 KiB, 35 | 2921.7 KiB, 804 | 290.1 KiB, 3 |
| random | 6 | 8197.4 KiB, 20 | 8226.8 KiB, 9 | 8193.1 KiB, 11 | 23345.0 KiB, 227 | 8226.1 KiB, 3 |
| random | 9e | 16393.4 KiB, 21 | 65574.2 KiB, 9 | 8193.1 KiB, 11 | 23343.5 KiB, 227 | 65574.1 KiB, 3 |
| zeros | 0 | 271.9 KiB, 106 | 290.8 KiB, 16 | 269.2 KiB, 43 | 2063.6 KiB, 764 | 290.1 KiB, 3 |
| zeros | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 16399.4 KiB, 101 | 8226.1 KiB, 3 |
| zeros | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 16399.4 KiB, 101 | 65574.1 KiB, 3 |
| pattern | 0 | 271.9 KiB, 106 | 290.8 KiB, 16 | 269.2 KiB, 43 | 2063.6 KiB, 764 | 290.1 KiB, 3 |
| pattern | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 16399.4 KiB, 101 | 8226.1 KiB, 3 |
| pattern | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 16399.4 KiB, 101 | 65574.1 KiB, 3 |
| far | 0 | 261.4 KiB, 98 | 290.8 KiB, 16 | 257.2 KiB, 35 | 2921.6 KiB, 804 | 290.1 KiB, 3 |
| far | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 23353.9 KiB, 158 | 8226.1 KiB, 3 |
| far | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 23353.9 KiB, 158 | 65574.1 KiB, 3 |

#### Silesia

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| dickens | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2063.6 KiB, 955 | 290.1 KiB, 3 |
| dickens | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26353.0 KiB, 102 | 8226.1 KiB, 3 |
| dickens | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26352.9 KiB, 102 | 65574.1 KiB, 3 |
| mozilla | 0 | 271.9 KiB, 639 | 290.8 KiB, 57 | 270.1 KiB, 251 | 2945.7 KiB, 4653 | 290.1 KiB, 3 |
| mozilla | 6 | 8207.9 KiB, 56 | 8226.8 KiB, 11 | 8205.1 KiB, 29 | 57363.4 KiB, 299 | 8226.1 KiB, 3 |
| mozilla | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 115579.4 KiB, 105 | 65574.1 KiB, 3 |
| mr | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2063.6 KiB, 955 | 290.1 KiB, 3 |
| mr | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26136.3 KiB, 102 | 8226.1 KiB, 3 |
| mr | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26136.3 KiB, 102 | 65574.1 KiB, 3 |
| nci | 0 | 271.9 KiB, 418 | 290.8 KiB, 40 | 269.6 KiB, 165 | 2064.0 KiB, 3046 | 290.1 KiB, 3 |
| nci | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57363.4 KiB, 201 | 8226.1 KiB, 3 |
| nci | 9e | 32787.9 KiB, 23 | 65574.2 KiB, 9 | 32785.1 KiB, 14 | 65554.5 KiB, 103 | 65574.1 KiB, 3 |
| ooffice | 0 | 271.9 KiB, 80 | 290.8 KiB, 14 | 269.2 KiB, 33 | 2063.5 KiB, 574 | 290.1 KiB, 3 |
| ooffice | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 14215.4 KiB, 101 | 8226.1 KiB, 3 |
| ooffice | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 14215.4 KiB, 101 | 65574.1 KiB, 3 |
| osdb | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2063.6 KiB, 955 | 290.1 KiB, 3 |
| osdb | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26248.7 KiB, 102 | 8226.1 KiB, 3 |
| osdb | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26248.8 KiB, 102 | 65574.1 KiB, 3 |
| reymont | 0 | 271.9 KiB, 93 | 290.8 KiB, 15 | 269.2 KiB, 38 | 2063.6 KiB, 668 | 290.1 KiB, 3 |
| reymont | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 14679.3 KiB, 101 | 8226.1 KiB, 3 |
| reymont | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 14679.3 KiB, 101 | 65574.1 KiB, 3 |
| samba | 0 | 271.9 KiB, 275 | 290.8 KiB, 29 | 269.6 KiB, 110 | 2063.9 KiB, 2001 | 290.1 KiB, 3 |
| samba | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 53887.4 KiB, 103 | 8226.1 KiB, 3 |
| samba | 9e | 32787.9 KiB, 23 | 65574.2 KiB, 9 | 32785.1 KiB, 14 | 53887.4 KiB, 103 | 65574.1 KiB, 3 |
| sao | 0 | 271.9 KiB, 93 | 290.8 KiB, 15 | 269.2 KiB, 38 | 2063.6 KiB, 669 | 290.1 KiB, 3 |
| sao | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 15289.4 KiB, 101 | 8226.1 KiB, 3 |
| sao | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 15289.4 KiB, 101 | 65574.1 KiB, 3 |
| webster | 0 | 271.9 KiB, 522 | 290.8 KiB, 48 | 270.1 KiB, 206 | 2064.5 KiB, 3807 | 290.1 KiB, 3 |
| webster | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57363.4 KiB, 202 | 8226.1 KiB, 3 |
| webster | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 106043.4 KiB, 104 | 65574.1 KiB, 3 |
| xml | 0 | 271.9 KiB, 79 | 290.8 KiB, 14 | 269.2 KiB, 32 | 2063.5 KiB, 571 | 290.1 KiB, 3 |
| xml | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 13427.4 KiB, 101 | 8226.1 KiB, 3 |
| xml | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 13427.4 KiB, 101 | 65574.1 KiB, 3 |
| x-ray | 0 | 271.9 KiB, 118 | 290.8 KiB, 17 | 269.2 KiB, 48 | 2063.6 KiB, 857 | 290.1 KiB, 3 |
| x-ray | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 24675.0 KiB, 102 | 8226.1 KiB, 3 |
| x-ray | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 24675.0 KiB, 102 | 65574.1 KiB, 3 |

#### Large files

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 0 | 271.9 KiB, 1250 | 290.8 KiB, 104 | 271.1 KiB, 487 | 2065.4 KiB, 9127 | 290.1 KiB, 3 |
| enwik8 | 6 | 8207.9 KiB, 78 | 8226.8 KiB, 12 | 8205.1 KiB, 42 | 57363.4 KiB, 403 | 8226.1 KiB, 3 |
| enwik8 | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 228751.3 KiB, 105 | 65574.1 KiB, 3 |
| linux-6.12.tar | 0 | 271.9 KiB, 1250 | 290.8 KiB, 104 | 271.1 KiB, 487 | 2065.4 KiB, 9127 | 290.1 KiB, 3 |
| linux-6.12.tar | 6 | 8207.9 KiB, 78 | 8226.8 KiB, 12 | 8205.1 KiB, 42 | 57359.4 KiB, 403 | 8226.1 KiB, 3 |
| linux-6.12.tar | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 228751.3 KiB, 105 | 65574.1 KiB, 3 |

#### Canterbury

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 419.9 KiB, 96 | 290.1 KiB, 3 |
| alice29.txt | 6 | 271.9 KiB, 16 | 8226.8 KiB, 9 | 269.1 KiB, 7 | 420.0 KiB, 96 | 8226.1 KiB, 3 |
| alice29.txt | 9e | 271.9 KiB, 16 | 65574.2 KiB, 9 | 269.1 KiB, 7 | 420.0 KiB, 96 | 65574.1 KiB, 3 |
| asyoulik.txt | 0 | 143.9 KiB, 14 | 290.8 KiB, 9 | 141.1 KiB, 6 | 265.7 KiB, 95 | 290.1 KiB, 3 |
| asyoulik.txt | 6 | 143.9 KiB, 15 | 8226.8 KiB, 9 | 141.1 KiB, 6 | 265.7 KiB, 95 | 8226.1 KiB, 3 |
| asyoulik.txt | 9e | 143.9 KiB, 15 | 65574.2 KiB, 9 | 141.1 KiB, 6 | 265.7 KiB, 95 | 65574.1 KiB, 3 |
| cp.html | 0 | 47.9 KiB, 12 | 290.8 KiB, 9 | 77.1 KiB, 5 | 71.4 KiB, 93 | 290.1 KiB, 3 |
| cp.html | 6 | 47.9 KiB, 13 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 71.5 KiB, 93 | 8226.1 KiB, 3 |
| cp.html | 9e | 47.9 KiB, 13 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 71.4 KiB, 93 | 65574.1 KiB, 3 |
| fields.c | 0 | 31.9 KiB, 11 | 290.8 KiB, 9 | 77.1 KiB, 5 | 42.3 KiB, 92 | 290.1 KiB, 3 |
| fields.c | 6 | 31.9 KiB, 12 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 42.3 KiB, 92 | 8226.1 KiB, 3 |
| fields.c | 9e | 31.9 KiB, 12 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 42.3 KiB, 92 | 65574.1 KiB, 3 |
| grammar.lsp | 0 | 19.9 KiB, 9 | 290.8 KiB, 9 | 77.1 KiB, 5 | 23.0 KiB, 90 | 290.1 KiB, 3 |
| grammar.lsp | 6 | 19.9 KiB, 10 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 23.0 KiB, 90 | 8226.1 KiB, 3 |
| grammar.lsp | 9e | 19.9 KiB, 10 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 23.0 KiB, 90 | 65574.1 KiB, 3 |
| kennedy.xls | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 2045.0 KiB, 98 | 290.1 KiB, 3 |
| kennedy.xls | 6 | 1039.9 KiB, 18 | 8226.8 KiB, 9 | 1037.1 KiB, 9 | 2045.0 KiB, 98 | 8226.1 KiB, 3 |
| kennedy.xls | 9e | 1039.9 KiB, 18 | 65574.2 KiB, 9 | 1037.1 KiB, 9 | 2045.0 KiB, 98 | 65574.1 KiB, 3 |
| lcet10.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 944.1 KiB, 97 | 290.1 KiB, 3 |
| lcet10.txt | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 944.1 KiB, 97 | 8226.1 KiB, 3 |
| lcet10.txt | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 944.2 KiB, 97 | 65574.1 KiB, 3 |
| plrabn12.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 998.0 KiB, 97 | 290.1 KiB, 3 |
| plrabn12.txt | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 998.0 KiB, 97 | 8226.1 KiB, 3 |
| plrabn12.txt | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 997.9 KiB, 97 | 65574.1 KiB, 3 |
| ptt5 | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1028.6 KiB, 97 | 290.1 KiB, 3 |
| ptt5 | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 1028.6 KiB, 97 | 8226.1 KiB, 3 |
| ptt5 | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 1028.6 KiB, 97 | 65574.1 KiB, 3 |
| sum | 0 | 79.9 KiB, 13 | 290.8 KiB, 9 | 77.1 KiB, 5 | 116.7 KiB, 94 | 290.1 KiB, 3 |
| sum | 6 | 79.9 KiB, 14 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 116.7 KiB, 94 | 8226.1 KiB, 3 |
| sum | 9e | 79.9 KiB, 14 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 116.7 KiB, 94 | 65574.1 KiB, 3 |
| xargs.1 | 0 | 23.9 KiB, 10 | 290.8 KiB, 9 | 77.1 KiB, 5 | 27.5 KiB, 91 | 290.1 KiB, 3 |
| xargs.1 | 6 | 23.9 KiB, 11 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 27.5 KiB, 91 | 8226.1 KiB, 3 |
| xargs.1 | 9e | 23.9 KiB, 11 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 27.5 KiB, 91 | 65574.1 KiB, 3 |
| E.coli | 0 | 271.9 KiB, 67 | 290.8 KiB, 13 | 269.2 KiB, 28 | 2063.5 KiB, 478 | 290.1 KiB, 3 |
| E.coli | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 12737.4 KiB, 101 | 8226.1 KiB, 3 |
| E.coli | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 12737.3 KiB, 101 | 65574.1 KiB, 3 |
| bible.txt | 0 | 271.9 KiB, 54 | 290.8 KiB, 12 | 269.1 KiB, 22 | 2063.4 KiB, 383 | 290.1 KiB, 3 |
| bible.txt | 6 | 4111.9 KiB, 20 | 8226.8 KiB, 9 | 4109.1 KiB, 11 | 8063.9 KiB, 100 | 8226.1 KiB, 3 |
| bible.txt | 9e | 4111.9 KiB, 20 | 65574.2 KiB, 9 | 4109.1 KiB, 11 | 8063.9 KiB, 100 | 65574.1 KiB, 3 |
| world192.txt | 0 | 271.9 KiB, 41 | 290.8 KiB, 11 | 269.1 KiB, 17 | 2063.4 KiB, 287 | 290.1 KiB, 3 |
| world192.txt | 6 | 4111.9 KiB, 20 | 8226.8 KiB, 9 | 4109.1 KiB, 11 | 6526.8 KiB, 100 | 8226.1 KiB, 3 |
| world192.txt | 9e | 4111.9 KiB, 20 | 65574.2 KiB, 9 | 4109.1 KiB, 11 | 6526.7 KiB, 100 | 65574.1 KiB, 3 |

#### Rust's executables

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rustc | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1670.5 KiB, 98 | 290.1 KiB, 3 |
| rustc | 6 | 1039.9 KiB, 18 | 8226.8 KiB, 9 | 1037.1 KiB, 9 | 1670.5 KiB, 98 | 8226.1 KiB, 3 |
| rustc | 9e | 1039.9 KiB, 18 | 65574.2 KiB, 9 | 1037.1 KiB, 9 | 1670.5 KiB, 98 | 65574.1 KiB, 3 |
| librustc_driver.so | 0 | 271.9 KiB, 1965 | 290.8 KiB, 159 | 273.1 KiB, 763 | 2067.4 KiB, 14355 | 290.1 KiB, 3 |
| librustc_driver.so | 6 | 8207.9 KiB, 135 | 8226.8 KiB, 15 | 8205.2 KiB, 73 | 57363.5 KiB, 703 | 8226.1 KiB, 3 |
| librustc_driver.so | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 416343.3 KiB, 107 | 65574.1 KiB, 3 |
| cargo | 0 | 271.9 KiB, 535 | 290.8 KiB, 49 | 270.1 KiB, 211 | 2064.4 KiB, 3904 | 290.1 KiB, 3 |
| cargo | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57363.3 KiB, 205 | 8226.1 KiB, 3 |
| cargo | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 107399.3 KiB, 106 | 65574.1 KiB, 3 |

Ours relative to liblzma's peak: 0.94x at the median, and at most 1.00x.

### ARM Neoverse-V3 (1 vCPU under EC2 Nitro), in pieces of 4 KiB

Measured 2026-11-21, at `b66411c`, `decode-xz` 0.0.4.

The size of each decoder's value: ours 336 B, liblzma 136 B, lzma-rust2
4440 B, lzma-rs 0 B, xz4rust 8 B.

#### Synthetic

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| random | 0 | 261.4 KiB, 98 | 290.8 KiB, 16 | 257.2 KiB, 35 | 2925.7 KiB, 805 | 290.1 KiB, 3 |
| random | 6 | 8197.4 KiB, 20 | 8226.8 KiB, 9 | 8193.1 KiB, 11 | 23349.0 KiB, 228 | 8226.1 KiB, 3 |
| random | 9e | 16393.4 KiB, 21 | 65574.2 KiB, 9 | 8193.1 KiB, 11 | 23347.5 KiB, 228 | 65574.1 KiB, 3 |
| zeros | 0 | 271.9 KiB, 106 | 290.8 KiB, 16 | 269.2 KiB, 43 | 2067.6 KiB, 765 | 290.1 KiB, 3 |
| zeros | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 16403.5 KiB, 102 | 8226.1 KiB, 3 |
| zeros | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 16403.4 KiB, 102 | 65574.1 KiB, 3 |
| pattern | 0 | 271.9 KiB, 106 | 290.8 KiB, 16 | 269.2 KiB, 43 | 2067.6 KiB, 765 | 290.1 KiB, 3 |
| pattern | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 16403.4 KiB, 102 | 8226.1 KiB, 3 |
| pattern | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 16403.4 KiB, 102 | 65574.1 KiB, 3 |
| far | 0 | 261.4 KiB, 98 | 290.8 KiB, 16 | 257.2 KiB, 35 | 2925.6 KiB, 805 | 290.1 KiB, 3 |
| far | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 23357.9 KiB, 159 | 8226.1 KiB, 3 |
| far | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 23357.9 KiB, 159 | 65574.1 KiB, 3 |

#### Silesia

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| dickens | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2067.5 KiB, 956 | 290.1 KiB, 3 |
| dickens | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26357.0 KiB, 103 | 8226.1 KiB, 3 |
| dickens | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26357.0 KiB, 103 | 65574.1 KiB, 3 |
| mozilla | 0 | 271.9 KiB, 639 | 290.8 KiB, 57 | 270.1 KiB, 251 | 2949.7 KiB, 4654 | 290.1 KiB, 3 |
| mozilla | 6 | 8207.9 KiB, 56 | 8226.8 KiB, 11 | 8205.1 KiB, 29 | 57367.5 KiB, 300 | 8226.1 KiB, 3 |
| mozilla | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 115583.4 KiB, 106 | 65574.1 KiB, 3 |
| mr | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2067.5 KiB, 956 | 290.1 KiB, 3 |
| mr | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26140.3 KiB, 103 | 8226.1 KiB, 3 |
| mr | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26140.3 KiB, 103 | 65574.1 KiB, 3 |
| nci | 0 | 271.9 KiB, 418 | 290.8 KiB, 40 | 269.6 KiB, 165 | 2067.9 KiB, 3047 | 290.1 KiB, 3 |
| nci | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57367.4 KiB, 202 | 8226.1 KiB, 3 |
| nci | 9e | 32787.9 KiB, 23 | 65574.2 KiB, 9 | 32785.1 KiB, 14 | 65558.4 KiB, 104 | 65574.1 KiB, 3 |
| ooffice | 0 | 271.9 KiB, 80 | 290.8 KiB, 14 | 269.2 KiB, 33 | 2067.5 KiB, 575 | 290.1 KiB, 3 |
| ooffice | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 14219.4 KiB, 102 | 8226.1 KiB, 3 |
| ooffice | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 14219.4 KiB, 102 | 65574.1 KiB, 3 |
| osdb | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2067.5 KiB, 956 | 290.1 KiB, 3 |
| osdb | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26252.7 KiB, 103 | 8226.1 KiB, 3 |
| osdb | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26252.7 KiB, 103 | 65574.1 KiB, 3 |
| reymont | 0 | 271.9 KiB, 93 | 290.8 KiB, 15 | 269.2 KiB, 38 | 2067.6 KiB, 669 | 290.1 KiB, 3 |
| reymont | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 14683.3 KiB, 102 | 8226.1 KiB, 3 |
| reymont | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 14683.3 KiB, 102 | 65574.1 KiB, 3 |
| samba | 0 | 271.9 KiB, 275 | 290.8 KiB, 29 | 269.6 KiB, 110 | 2068.0 KiB, 2002 | 290.1 KiB, 3 |
| samba | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 53891.4 KiB, 104 | 8226.1 KiB, 3 |
| samba | 9e | 32787.9 KiB, 23 | 65574.2 KiB, 9 | 32785.1 KiB, 14 | 53891.4 KiB, 104 | 65574.1 KiB, 3 |
| sao | 0 | 271.9 KiB, 93 | 290.8 KiB, 15 | 269.2 KiB, 38 | 2067.5 KiB, 670 | 290.1 KiB, 3 |
| sao | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 15293.4 KiB, 102 | 8226.1 KiB, 3 |
| sao | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 15293.4 KiB, 102 | 65574.1 KiB, 3 |
| webster | 0 | 271.9 KiB, 522 | 290.8 KiB, 48 | 270.1 KiB, 206 | 2068.5 KiB, 3808 | 290.1 KiB, 3 |
| webster | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57367.4 KiB, 203 | 8226.1 KiB, 3 |
| webster | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 106047.3 KiB, 105 | 65574.1 KiB, 3 |
| xml | 0 | 271.9 KiB, 79 | 290.8 KiB, 14 | 269.2 KiB, 32 | 2067.5 KiB, 572 | 290.1 KiB, 3 |
| xml | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 13431.4 KiB, 102 | 8226.1 KiB, 3 |
| xml | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 13431.4 KiB, 102 | 65574.1 KiB, 3 |
| x-ray | 0 | 271.9 KiB, 118 | 290.8 KiB, 17 | 269.2 KiB, 48 | 2067.6 KiB, 858 | 290.1 KiB, 3 |
| x-ray | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 24679.0 KiB, 103 | 8226.1 KiB, 3 |
| x-ray | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 24679.0 KiB, 103 | 65574.1 KiB, 3 |

#### Large files

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 0 | 271.9 KiB, 1250 | 290.8 KiB, 104 | 271.1 KiB, 487 | 2069.4 KiB, 9128 | 290.1 KiB, 3 |
| enwik8 | 6 | 8207.9 KiB, 78 | 8226.8 KiB, 12 | 8205.1 KiB, 42 | 57367.4 KiB, 404 | 8226.1 KiB, 3 |
| enwik8 | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 228755.4 KiB, 106 | 65574.1 KiB, 3 |
| linux-6.12.tar | 0 | 271.9 KiB, 1250 | 290.8 KiB, 104 | 271.1 KiB, 487 | 2069.5 KiB, 9128 | 290.1 KiB, 3 |
| linux-6.12.tar | 6 | 8207.9 KiB, 78 | 8226.8 KiB, 12 | 8205.1 KiB, 42 | 57363.5 KiB, 404 | 8226.1 KiB, 3 |
| linux-6.12.tar | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 228755.4 KiB, 106 | 65574.1 KiB, 3 |

#### Canterbury

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 424.0 KiB, 97 | 290.1 KiB, 3 |
| alice29.txt | 6 | 271.9 KiB, 16 | 8226.8 KiB, 9 | 269.1 KiB, 7 | 424.0 KiB, 97 | 8226.1 KiB, 3 |
| alice29.txt | 9e | 271.9 KiB, 16 | 65574.2 KiB, 9 | 269.1 KiB, 7 | 423.9 KiB, 97 | 65574.1 KiB, 3 |
| asyoulik.txt | 0 | 143.9 KiB, 14 | 290.8 KiB, 9 | 141.1 KiB, 6 | 269.7 KiB, 96 | 290.1 KiB, 3 |
| asyoulik.txt | 6 | 143.9 KiB, 15 | 8226.8 KiB, 9 | 141.1 KiB, 6 | 269.7 KiB, 96 | 8226.1 KiB, 3 |
| asyoulik.txt | 9e | 143.9 KiB, 15 | 65574.2 KiB, 9 | 141.1 KiB, 6 | 269.7 KiB, 96 | 65574.1 KiB, 3 |
| cp.html | 0 | 47.9 KiB, 12 | 290.8 KiB, 9 | 77.1 KiB, 5 | 75.5 KiB, 94 | 290.1 KiB, 3 |
| cp.html | 6 | 47.9 KiB, 13 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 75.5 KiB, 94 | 8226.1 KiB, 3 |
| cp.html | 9e | 47.9 KiB, 13 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 75.4 KiB, 94 | 65574.1 KiB, 3 |
| fields.c | 0 | 31.9 KiB, 11 | 290.8 KiB, 9 | 77.1 KiB, 5 | 46.3 KiB, 93 | 290.1 KiB, 3 |
| fields.c | 6 | 31.9 KiB, 12 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 46.3 KiB, 93 | 8226.1 KiB, 3 |
| fields.c | 9e | 31.9 KiB, 12 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 46.3 KiB, 93 | 65574.1 KiB, 3 |
| grammar.lsp | 0 | 19.9 KiB, 9 | 290.8 KiB, 9 | 77.1 KiB, 5 | 27.0 KiB, 91 | 290.1 KiB, 3 |
| grammar.lsp | 6 | 19.9 KiB, 10 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 27.1 KiB, 91 | 8226.1 KiB, 3 |
| grammar.lsp | 9e | 19.9 KiB, 10 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 27.1 KiB, 91 | 65574.1 KiB, 3 |
| kennedy.xls | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 2049.0 KiB, 99 | 290.1 KiB, 3 |
| kennedy.xls | 6 | 1039.9 KiB, 18 | 8226.8 KiB, 9 | 1037.1 KiB, 9 | 2049.0 KiB, 99 | 8226.1 KiB, 3 |
| kennedy.xls | 9e | 1039.9 KiB, 18 | 65574.2 KiB, 9 | 1037.1 KiB, 9 | 2049.0 KiB, 99 | 65574.1 KiB, 3 |
| lcet10.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 948.1 KiB, 98 | 290.1 KiB, 3 |
| lcet10.txt | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 948.1 KiB, 98 | 8226.1 KiB, 3 |
| lcet10.txt | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 948.1 KiB, 98 | 65574.1 KiB, 3 |
| plrabn12.txt | 0 | 272.0 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1002.0 KiB, 98 | 290.1 KiB, 3 |
| plrabn12.txt | 6 | 528.0 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 1002.0 KiB, 98 | 8226.1 KiB, 3 |
| plrabn12.txt | 9e | 528.0 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 1002.0 KiB, 98 | 65574.1 KiB, 3 |
| ptt5 | 0 | 272.0 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1032.6 KiB, 98 | 290.1 KiB, 3 |
| ptt5 | 6 | 528.0 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 1032.6 KiB, 98 | 8226.1 KiB, 3 |
| ptt5 | 9e | 528.0 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 1032.6 KiB, 98 | 65574.1 KiB, 3 |
| sum | 0 | 80.0 KiB, 13 | 290.8 KiB, 9 | 77.1 KiB, 5 | 120.8 KiB, 95 | 290.1 KiB, 3 |
| sum | 6 | 80.0 KiB, 14 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 120.7 KiB, 95 | 8226.1 KiB, 3 |
| sum | 9e | 79.9 KiB, 14 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 120.7 KiB, 95 | 65574.1 KiB, 3 |
| xargs.1 | 0 | 23.9 KiB, 10 | 290.8 KiB, 9 | 77.1 KiB, 5 | 31.5 KiB, 92 | 290.1 KiB, 3 |
| xargs.1 | 6 | 23.9 KiB, 11 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 31.5 KiB, 92 | 8226.1 KiB, 3 |
| xargs.1 | 9e | 23.9 KiB, 11 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 31.5 KiB, 92 | 65574.1 KiB, 3 |
| E.coli | 0 | 271.9 KiB, 67 | 290.8 KiB, 13 | 269.2 KiB, 28 | 2067.5 KiB, 479 | 290.1 KiB, 3 |
| E.coli | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 12741.4 KiB, 102 | 8226.1 KiB, 3 |
| E.coli | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 12741.4 KiB, 102 | 65574.1 KiB, 3 |
| bible.txt | 0 | 271.9 KiB, 54 | 290.8 KiB, 12 | 269.1 KiB, 22 | 2067.5 KiB, 384 | 290.1 KiB, 3 |
| bible.txt | 6 | 4111.9 KiB, 20 | 8226.8 KiB, 9 | 4109.1 KiB, 11 | 8067.9 KiB, 101 | 8226.1 KiB, 3 |
| bible.txt | 9e | 4111.9 KiB, 20 | 65574.2 KiB, 9 | 4109.1 KiB, 11 | 8067.9 KiB, 101 | 65574.1 KiB, 3 |
| world192.txt | 0 | 271.9 KiB, 41 | 290.8 KiB, 11 | 269.1 KiB, 17 | 2067.5 KiB, 288 | 290.1 KiB, 3 |
| world192.txt | 6 | 4111.9 KiB, 20 | 8226.8 KiB, 9 | 4109.1 KiB, 11 | 6530.9 KiB, 101 | 8226.1 KiB, 3 |
| world192.txt | 9e | 4111.9 KiB, 20 | 65574.2 KiB, 9 | 4109.1 KiB, 11 | 6530.9 KiB, 101 | 65574.1 KiB, 3 |

#### Rust's executables

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rustc | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1674.6 KiB, 99 | 290.1 KiB, 3 |
| rustc | 6 | 1039.9 KiB, 18 | 8226.8 KiB, 9 | 1037.1 KiB, 9 | 1674.6 KiB, 99 | 8226.1 KiB, 3 |
| rustc | 9e | 1039.9 KiB, 18 | 65574.2 KiB, 9 | 1037.1 KiB, 9 | 1674.6 KiB, 99 | 65574.1 KiB, 3 |
| librustc_driver.so | 0 | 271.9 KiB, 1965 | 290.8 KiB, 159 | 273.1 KiB, 763 | 2071.5 KiB, 14356 | 290.1 KiB, 3 |
| librustc_driver.so | 6 | 8207.9 KiB, 135 | 8226.8 KiB, 15 | 8205.2 KiB, 73 | 57367.5 KiB, 704 | 8226.1 KiB, 3 |
| librustc_driver.so | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 416347.3 KiB, 108 | 65574.1 KiB, 3 |
| cargo | 0 | 271.9 KiB, 535 | 290.8 KiB, 49 | 270.1 KiB, 211 | 2068.5 KiB, 3905 | 290.1 KiB, 3 |
| cargo | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57367.4 KiB, 206 | 8226.1 KiB, 3 |
| cargo | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 107403.4 KiB, 107 | 65574.1 KiB, 3 |

Ours relative to liblzma's peak: 0.94x at the median, and at most 1.00x.

### ARM Neoverse-V3 (1 vCPU under EC2 Nitro), in pieces of 64 KiB

Measured 2026-11-21, at `b66411c`, `decode-xz` 0.0.4.

The size of each decoder's value: ours 336 B, liblzma 136 B, lzma-rust2
4440 B, lzma-rs 0 B, xz4rust 8 B.

#### Synthetic

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| random | 0 | 261.4 KiB, 98 | 290.8 KiB, 16 | 257.2 KiB, 35 | 2985.7 KiB, 805 | 290.1 KiB, 3 |
| random | 6 | 8197.4 KiB, 20 | 8226.8 KiB, 9 | 8193.1 KiB, 11 | 23409.0 KiB, 228 | 8226.1 KiB, 3 |
| random | 9e | 16393.4 KiB, 21 | 65574.2 KiB, 9 | 8193.1 KiB, 11 | 23407.5 KiB, 228 | 65574.1 KiB, 3 |
| zeros | 0 | 271.9 KiB, 106 | 290.8 KiB, 16 | 269.2 KiB, 43 | 2127.6 KiB, 765 | 290.1 KiB, 3 |
| zeros | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 16463.4 KiB, 102 | 8226.1 KiB, 3 |
| zeros | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 16463.4 KiB, 102 | 65574.1 KiB, 3 |
| pattern | 0 | 271.9 KiB, 106 | 290.8 KiB, 16 | 269.2 KiB, 43 | 2127.6 KiB, 765 | 290.1 KiB, 3 |
| pattern | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 16463.4 KiB, 102 | 8226.1 KiB, 3 |
| pattern | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 16463.4 KiB, 102 | 65574.1 KiB, 3 |
| far | 0 | 261.4 KiB, 98 | 290.8 KiB, 16 | 257.2 KiB, 35 | 2985.6 KiB, 805 | 290.1 KiB, 3 |
| far | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 23417.9 KiB, 159 | 8226.1 KiB, 3 |
| far | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 23417.9 KiB, 159 | 65574.1 KiB, 3 |

#### Silesia

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| dickens | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2127.6 KiB, 956 | 290.1 KiB, 3 |
| dickens | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26417.0 KiB, 103 | 8226.1 KiB, 3 |
| dickens | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26416.9 KiB, 103 | 65574.1 KiB, 3 |
| mozilla | 0 | 271.9 KiB, 639 | 290.8 KiB, 57 | 270.1 KiB, 251 | 3009.7 KiB, 4654 | 290.1 KiB, 3 |
| mozilla | 6 | 8207.9 KiB, 56 | 8226.8 KiB, 11 | 8205.1 KiB, 29 | 57427.5 KiB, 300 | 8226.1 KiB, 3 |
| mozilla | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 115643.4 KiB, 106 | 65574.1 KiB, 3 |
| mr | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2127.6 KiB, 956 | 290.1 KiB, 3 |
| mr | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26200.3 KiB, 103 | 8226.1 KiB, 3 |
| mr | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26200.3 KiB, 103 | 65574.1 KiB, 3 |
| nci | 0 | 271.9 KiB, 418 | 290.8 KiB, 40 | 269.6 KiB, 165 | 2128.0 KiB, 3047 | 290.1 KiB, 3 |
| nci | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57427.4 KiB, 202 | 8226.1 KiB, 3 |
| nci | 9e | 32787.9 KiB, 23 | 65574.2 KiB, 9 | 32785.1 KiB, 14 | 65618.5 KiB, 104 | 65574.1 KiB, 3 |
| ooffice | 0 | 271.9 KiB, 80 | 290.8 KiB, 14 | 269.2 KiB, 33 | 2127.5 KiB, 575 | 290.1 KiB, 3 |
| ooffice | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 14279.4 KiB, 102 | 8226.1 KiB, 3 |
| ooffice | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 14279.4 KiB, 102 | 65574.1 KiB, 3 |
| osdb | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2127.6 KiB, 956 | 290.1 KiB, 3 |
| osdb | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26312.7 KiB, 103 | 8226.1 KiB, 3 |
| osdb | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26312.8 KiB, 103 | 65574.1 KiB, 3 |
| reymont | 0 | 271.9 KiB, 93 | 290.8 KiB, 15 | 269.2 KiB, 38 | 2127.6 KiB, 669 | 290.1 KiB, 3 |
| reymont | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 14743.4 KiB, 102 | 8226.1 KiB, 3 |
| reymont | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 14743.3 KiB, 102 | 65574.1 KiB, 3 |
| samba | 0 | 271.9 KiB, 275 | 290.8 KiB, 29 | 269.6 KiB, 110 | 2128.0 KiB, 2002 | 290.1 KiB, 3 |
| samba | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 53951.4 KiB, 104 | 8226.1 KiB, 3 |
| samba | 9e | 32787.9 KiB, 23 | 65574.2 KiB, 9 | 32785.1 KiB, 14 | 53951.4 KiB, 104 | 65574.1 KiB, 3 |
| sao | 0 | 271.9 KiB, 93 | 290.8 KiB, 15 | 269.2 KiB, 38 | 2127.6 KiB, 670 | 290.1 KiB, 3 |
| sao | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 15353.4 KiB, 102 | 8226.1 KiB, 3 |
| sao | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 15353.4 KiB, 102 | 65574.1 KiB, 3 |
| webster | 0 | 271.9 KiB, 522 | 290.8 KiB, 48 | 270.1 KiB, 206 | 2128.5 KiB, 3808 | 290.1 KiB, 3 |
| webster | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57427.4 KiB, 203 | 8226.1 KiB, 3 |
| webster | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 106107.4 KiB, 105 | 65574.1 KiB, 3 |
| xml | 0 | 271.9 KiB, 79 | 290.8 KiB, 14 | 269.2 KiB, 32 | 2127.5 KiB, 572 | 290.1 KiB, 3 |
| xml | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 13491.4 KiB, 102 | 8226.1 KiB, 3 |
| xml | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 13491.4 KiB, 102 | 65574.1 KiB, 3 |
| x-ray | 0 | 271.9 KiB, 118 | 290.8 KiB, 17 | 269.2 KiB, 48 | 2127.6 KiB, 858 | 290.1 KiB, 3 |
| x-ray | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 24739.1 KiB, 103 | 8226.1 KiB, 3 |
| x-ray | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 24739.1 KiB, 103 | 65574.1 KiB, 3 |

#### Large files

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 0 | 271.9 KiB, 1250 | 290.8 KiB, 104 | 271.1 KiB, 487 | 2129.4 KiB, 9128 | 290.1 KiB, 3 |
| enwik8 | 6 | 8207.9 KiB, 78 | 8226.8 KiB, 12 | 8205.1 KiB, 42 | 57427.4 KiB, 404 | 8226.1 KiB, 3 |
| enwik8 | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 228815.3 KiB, 106 | 65574.1 KiB, 3 |
| linux-6.12.tar | 0 | 271.9 KiB, 1250 | 290.8 KiB, 104 | 271.1 KiB, 487 | 2129.4 KiB, 9128 | 290.1 KiB, 3 |
| linux-6.12.tar | 6 | 8207.9 KiB, 78 | 8226.8 KiB, 12 | 8205.1 KiB, 42 | 57423.4 KiB, 404 | 8226.1 KiB, 3 |
| linux-6.12.tar | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 228815.3 KiB, 106 | 65574.1 KiB, 3 |

#### Canterbury

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 483.9 KiB, 97 | 290.1 KiB, 3 |
| alice29.txt | 6 | 271.9 KiB, 16 | 8226.8 KiB, 9 | 269.1 KiB, 7 | 484.0 KiB, 97 | 8226.1 KiB, 3 |
| alice29.txt | 9e | 271.9 KiB, 16 | 65574.2 KiB, 9 | 269.1 KiB, 7 | 484.0 KiB, 97 | 65574.1 KiB, 3 |
| asyoulik.txt | 0 | 143.9 KiB, 14 | 290.8 KiB, 9 | 141.1 KiB, 6 | 329.7 KiB, 96 | 290.1 KiB, 3 |
| asyoulik.txt | 6 | 143.9 KiB, 15 | 8226.8 KiB, 9 | 141.1 KiB, 6 | 329.7 KiB, 96 | 8226.1 KiB, 3 |
| asyoulik.txt | 9e | 143.9 KiB, 15 | 65574.2 KiB, 9 | 141.1 KiB, 6 | 329.7 KiB, 96 | 65574.1 KiB, 3 |
| cp.html | 0 | 47.9 KiB, 12 | 290.8 KiB, 9 | 77.1 KiB, 5 | 135.4 KiB, 94 | 290.1 KiB, 3 |
| cp.html | 6 | 47.9 KiB, 13 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 135.5 KiB, 94 | 8226.1 KiB, 3 |
| cp.html | 9e | 47.9 KiB, 13 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 135.4 KiB, 94 | 65574.1 KiB, 3 |
| fields.c | 0 | 31.9 KiB, 11 | 290.8 KiB, 9 | 77.1 KiB, 5 | 106.3 KiB, 93 | 290.1 KiB, 3 |
| fields.c | 6 | 31.9 KiB, 12 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 106.3 KiB, 93 | 8226.1 KiB, 3 |
| fields.c | 9e | 31.9 KiB, 12 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 106.3 KiB, 93 | 65574.1 KiB, 3 |
| grammar.lsp | 0 | 19.9 KiB, 9 | 290.8 KiB, 9 | 77.1 KiB, 5 | 87.0 KiB, 91 | 290.1 KiB, 3 |
| grammar.lsp | 6 | 19.9 KiB, 10 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 87.0 KiB, 91 | 8226.1 KiB, 3 |
| grammar.lsp | 9e | 19.9 KiB, 10 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 87.0 KiB, 91 | 65574.1 KiB, 3 |
| kennedy.xls | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 2109.0 KiB, 99 | 290.1 KiB, 3 |
| kennedy.xls | 6 | 1039.9 KiB, 18 | 8226.8 KiB, 9 | 1037.1 KiB, 9 | 2109.0 KiB, 99 | 8226.1 KiB, 3 |
| kennedy.xls | 9e | 1039.9 KiB, 18 | 65574.2 KiB, 9 | 1037.1 KiB, 9 | 2109.0 KiB, 99 | 65574.1 KiB, 3 |
| lcet10.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1008.1 KiB, 98 | 290.1 KiB, 3 |
| lcet10.txt | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 1008.1 KiB, 98 | 8226.1 KiB, 3 |
| lcet10.txt | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 1008.2 KiB, 98 | 65574.1 KiB, 3 |
| plrabn12.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1062.0 KiB, 98 | 290.1 KiB, 3 |
| plrabn12.txt | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 1062.0 KiB, 98 | 8226.1 KiB, 3 |
| plrabn12.txt | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 1061.9 KiB, 98 | 65574.1 KiB, 3 |
| ptt5 | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1092.6 KiB, 98 | 290.1 KiB, 3 |
| ptt5 | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 1092.6 KiB, 98 | 8226.1 KiB, 3 |
| ptt5 | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 1092.6 KiB, 98 | 65574.1 KiB, 3 |
| sum | 0 | 79.9 KiB, 13 | 290.8 KiB, 9 | 77.1 KiB, 5 | 180.7 KiB, 95 | 290.1 KiB, 3 |
| sum | 6 | 79.9 KiB, 14 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 180.7 KiB, 95 | 8226.1 KiB, 3 |
| sum | 9e | 79.9 KiB, 14 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 180.7 KiB, 95 | 65574.1 KiB, 3 |
| xargs.1 | 0 | 23.9 KiB, 10 | 290.8 KiB, 9 | 77.1 KiB, 5 | 91.5 KiB, 92 | 290.1 KiB, 3 |
| xargs.1 | 6 | 23.9 KiB, 11 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 91.5 KiB, 92 | 8226.1 KiB, 3 |
| xargs.1 | 9e | 23.9 KiB, 11 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 91.5 KiB, 92 | 65574.1 KiB, 3 |
| E.coli | 0 | 271.9 KiB, 67 | 290.8 KiB, 13 | 269.2 KiB, 28 | 2127.5 KiB, 479 | 290.1 KiB, 3 |
| E.coli | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 12801.4 KiB, 102 | 8226.1 KiB, 3 |
| E.coli | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 12801.3 KiB, 102 | 65574.1 KiB, 3 |
| bible.txt | 0 | 271.9 KiB, 54 | 290.8 KiB, 12 | 269.1 KiB, 22 | 2127.4 KiB, 384 | 290.1 KiB, 3 |
| bible.txt | 6 | 4111.9 KiB, 20 | 8226.8 KiB, 9 | 4109.1 KiB, 11 | 8127.9 KiB, 101 | 8226.1 KiB, 3 |
| bible.txt | 9e | 4111.9 KiB, 20 | 65574.2 KiB, 9 | 4109.1 KiB, 11 | 8127.9 KiB, 101 | 65574.1 KiB, 3 |
| world192.txt | 0 | 271.9 KiB, 41 | 290.8 KiB, 11 | 269.1 KiB, 17 | 2127.4 KiB, 288 | 290.1 KiB, 3 |
| world192.txt | 6 | 4111.9 KiB, 20 | 8226.8 KiB, 9 | 4109.1 KiB, 11 | 6590.8 KiB, 101 | 8226.1 KiB, 3 |
| world192.txt | 9e | 4111.9 KiB, 20 | 65574.2 KiB, 9 | 4109.1 KiB, 11 | 6590.7 KiB, 101 | 65574.1 KiB, 3 |

#### Rust's executables

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rustc | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1734.6 KiB, 99 | 290.1 KiB, 3 |
| rustc | 6 | 1039.9 KiB, 18 | 8226.8 KiB, 9 | 1037.1 KiB, 9 | 1734.6 KiB, 99 | 8226.1 KiB, 3 |
| rustc | 9e | 1039.9 KiB, 18 | 65574.2 KiB, 9 | 1037.1 KiB, 9 | 1734.6 KiB, 99 | 65574.1 KiB, 3 |
| librustc_driver.so | 0 | 271.9 KiB, 1965 | 290.8 KiB, 159 | 273.1 KiB, 763 | 2131.4 KiB, 14356 | 290.1 KiB, 3 |
| librustc_driver.so | 6 | 8207.9 KiB, 135 | 8226.8 KiB, 15 | 8205.2 KiB, 73 | 57427.5 KiB, 704 | 8226.1 KiB, 3 |
| librustc_driver.so | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 416407.3 KiB, 108 | 65574.1 KiB, 3 |
| cargo | 0 | 271.9 KiB, 535 | 290.8 KiB, 49 | 270.1 KiB, 211 | 2128.4 KiB, 3905 | 290.1 KiB, 3 |
| cargo | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57427.4 KiB, 206 | 8226.1 KiB, 3 |
| cargo | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 107463.3 KiB, 107 | 65574.1 KiB, 3 |

Ours relative to liblzma's peak: 0.94x at the median, and at most 1.00x.

### Intel Xeon 6975P-C (1 vCPU under EC2 Nitro), whole

Measured 2026-11-21, at `b66411c`, `decode-xz` 0.0.4.

The size of each decoder's value: ours 336 B, liblzma 136 B, lzma-rust2
4440 B, lzma-rs 0 B, xz4rust 8 B.

#### Synthetic

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| random | 0 | 261.4 KiB, 98 | 290.8 KiB, 16 | 257.2 KiB, 35 | 2921.7 KiB, 804 | 290.1 KiB, 3 |
| random | 6 | 8197.4 KiB, 20 | 8226.8 KiB, 9 | 8193.1 KiB, 11 | 23345.0 KiB, 227 | 8226.1 KiB, 3 |
| random | 9e | 16393.4 KiB, 21 | 65574.2 KiB, 9 | 8193.1 KiB, 11 | 23343.5 KiB, 227 | 65574.1 KiB, 3 |
| zeros | 0 | 271.9 KiB, 106 | 290.8 KiB, 16 | 269.2 KiB, 43 | 2063.6 KiB, 764 | 290.1 KiB, 3 |
| zeros | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 16399.4 KiB, 101 | 8226.1 KiB, 3 |
| zeros | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 16399.4 KiB, 101 | 65574.1 KiB, 3 |
| pattern | 0 | 271.9 KiB, 106 | 290.8 KiB, 16 | 269.2 KiB, 43 | 2063.6 KiB, 764 | 290.1 KiB, 3 |
| pattern | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 16399.4 KiB, 101 | 8226.1 KiB, 3 |
| pattern | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 16399.4 KiB, 101 | 65574.1 KiB, 3 |
| far | 0 | 261.4 KiB, 98 | 290.8 KiB, 16 | 257.2 KiB, 35 | 2921.6 KiB, 804 | 290.1 KiB, 3 |
| far | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 23353.9 KiB, 158 | 8226.1 KiB, 3 |
| far | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 23353.9 KiB, 158 | 65574.1 KiB, 3 |

#### Silesia

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| dickens | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2063.6 KiB, 955 | 290.1 KiB, 3 |
| dickens | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26353.0 KiB, 102 | 8226.1 KiB, 3 |
| dickens | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26352.9 KiB, 102 | 65574.1 KiB, 3 |
| mozilla | 0 | 271.9 KiB, 639 | 290.8 KiB, 57 | 270.1 KiB, 251 | 2945.7 KiB, 4653 | 290.1 KiB, 3 |
| mozilla | 6 | 8207.9 KiB, 56 | 8226.8 KiB, 11 | 8205.1 KiB, 29 | 57363.4 KiB, 299 | 8226.1 KiB, 3 |
| mozilla | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 115579.4 KiB, 105 | 65574.1 KiB, 3 |
| mr | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2063.6 KiB, 955 | 290.1 KiB, 3 |
| mr | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26136.3 KiB, 102 | 8226.1 KiB, 3 |
| mr | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26136.3 KiB, 102 | 65574.1 KiB, 3 |
| nci | 0 | 271.9 KiB, 418 | 290.8 KiB, 40 | 269.6 KiB, 165 | 2064.0 KiB, 3046 | 290.1 KiB, 3 |
| nci | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57363.4 KiB, 201 | 8226.1 KiB, 3 |
| nci | 9e | 32787.9 KiB, 23 | 65574.2 KiB, 9 | 32785.1 KiB, 14 | 65554.5 KiB, 103 | 65574.1 KiB, 3 |
| ooffice | 0 | 271.9 KiB, 80 | 290.8 KiB, 14 | 269.2 KiB, 33 | 2063.5 KiB, 574 | 290.1 KiB, 3 |
| ooffice | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 14215.4 KiB, 101 | 8226.1 KiB, 3 |
| ooffice | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 14215.4 KiB, 101 | 65574.1 KiB, 3 |
| osdb | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2063.6 KiB, 955 | 290.1 KiB, 3 |
| osdb | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26248.7 KiB, 102 | 8226.1 KiB, 3 |
| osdb | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26248.8 KiB, 102 | 65574.1 KiB, 3 |
| reymont | 0 | 271.9 KiB, 93 | 290.8 KiB, 15 | 269.2 KiB, 38 | 2063.6 KiB, 668 | 290.1 KiB, 3 |
| reymont | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 14679.3 KiB, 101 | 8226.1 KiB, 3 |
| reymont | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 14679.3 KiB, 101 | 65574.1 KiB, 3 |
| samba | 0 | 271.9 KiB, 275 | 290.8 KiB, 29 | 269.6 KiB, 110 | 2063.9 KiB, 2001 | 290.1 KiB, 3 |
| samba | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 53887.4 KiB, 103 | 8226.1 KiB, 3 |
| samba | 9e | 32787.9 KiB, 23 | 65574.2 KiB, 9 | 32785.1 KiB, 14 | 53887.4 KiB, 103 | 65574.1 KiB, 3 |
| sao | 0 | 271.9 KiB, 93 | 290.8 KiB, 15 | 269.2 KiB, 38 | 2063.6 KiB, 669 | 290.1 KiB, 3 |
| sao | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 15289.4 KiB, 101 | 8226.1 KiB, 3 |
| sao | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 15289.4 KiB, 101 | 65574.1 KiB, 3 |
| webster | 0 | 271.9 KiB, 522 | 290.8 KiB, 48 | 270.1 KiB, 206 | 2064.5 KiB, 3807 | 290.1 KiB, 3 |
| webster | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57363.4 KiB, 202 | 8226.1 KiB, 3 |
| webster | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 106043.4 KiB, 104 | 65574.1 KiB, 3 |
| xml | 0 | 271.9 KiB, 79 | 290.8 KiB, 14 | 269.2 KiB, 32 | 2063.5 KiB, 571 | 290.1 KiB, 3 |
| xml | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 13427.4 KiB, 101 | 8226.1 KiB, 3 |
| xml | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 13427.4 KiB, 101 | 65574.1 KiB, 3 |
| x-ray | 0 | 271.9 KiB, 118 | 290.8 KiB, 17 | 269.2 KiB, 48 | 2063.6 KiB, 857 | 290.1 KiB, 3 |
| x-ray | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 24675.0 KiB, 102 | 8226.1 KiB, 3 |
| x-ray | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 24675.0 KiB, 102 | 65574.1 KiB, 3 |

#### Large files

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 0 | 271.9 KiB, 1250 | 290.8 KiB, 104 | 271.1 KiB, 487 | 2065.4 KiB, 9127 | 290.1 KiB, 3 |
| enwik8 | 6 | 8207.9 KiB, 78 | 8226.8 KiB, 12 | 8205.1 KiB, 42 | 57363.4 KiB, 403 | 8226.1 KiB, 3 |
| enwik8 | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 228751.3 KiB, 105 | 65574.1 KiB, 3 |
| linux-6.12.tar | 0 | 271.9 KiB, 1250 | 290.8 KiB, 104 | 271.1 KiB, 487 | 2065.4 KiB, 9127 | 290.1 KiB, 3 |
| linux-6.12.tar | 6 | 8207.9 KiB, 78 | 8226.8 KiB, 12 | 8205.1 KiB, 42 | 57359.4 KiB, 403 | 8226.1 KiB, 3 |
| linux-6.12.tar | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 228751.3 KiB, 105 | 65574.1 KiB, 3 |

#### Canterbury

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 419.9 KiB, 96 | 290.1 KiB, 3 |
| alice29.txt | 6 | 271.9 KiB, 16 | 8226.8 KiB, 9 | 269.1 KiB, 7 | 420.0 KiB, 96 | 8226.1 KiB, 3 |
| alice29.txt | 9e | 271.9 KiB, 16 | 65574.2 KiB, 9 | 269.1 KiB, 7 | 420.0 KiB, 96 | 65574.1 KiB, 3 |
| asyoulik.txt | 0 | 143.9 KiB, 14 | 290.8 KiB, 9 | 141.1 KiB, 6 | 265.7 KiB, 95 | 290.1 KiB, 3 |
| asyoulik.txt | 6 | 143.9 KiB, 15 | 8226.8 KiB, 9 | 141.1 KiB, 6 | 265.7 KiB, 95 | 8226.1 KiB, 3 |
| asyoulik.txt | 9e | 143.9 KiB, 15 | 65574.2 KiB, 9 | 141.1 KiB, 6 | 265.7 KiB, 95 | 65574.1 KiB, 3 |
| cp.html | 0 | 47.9 KiB, 12 | 290.8 KiB, 9 | 77.1 KiB, 5 | 71.4 KiB, 93 | 290.1 KiB, 3 |
| cp.html | 6 | 47.9 KiB, 13 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 71.5 KiB, 93 | 8226.1 KiB, 3 |
| cp.html | 9e | 47.9 KiB, 13 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 71.4 KiB, 93 | 65574.1 KiB, 3 |
| fields.c | 0 | 31.9 KiB, 11 | 290.8 KiB, 9 | 77.1 KiB, 5 | 42.3 KiB, 92 | 290.1 KiB, 3 |
| fields.c | 6 | 31.9 KiB, 12 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 42.3 KiB, 92 | 8226.1 KiB, 3 |
| fields.c | 9e | 31.9 KiB, 12 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 42.3 KiB, 92 | 65574.1 KiB, 3 |
| grammar.lsp | 0 | 19.9 KiB, 9 | 290.8 KiB, 9 | 77.1 KiB, 5 | 23.0 KiB, 90 | 290.1 KiB, 3 |
| grammar.lsp | 6 | 19.9 KiB, 10 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 23.0 KiB, 90 | 8226.1 KiB, 3 |
| grammar.lsp | 9e | 19.9 KiB, 10 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 23.0 KiB, 90 | 65574.1 KiB, 3 |
| kennedy.xls | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 2045.0 KiB, 98 | 290.1 KiB, 3 |
| kennedy.xls | 6 | 1039.9 KiB, 18 | 8226.8 KiB, 9 | 1037.1 KiB, 9 | 2045.0 KiB, 98 | 8226.1 KiB, 3 |
| kennedy.xls | 9e | 1039.9 KiB, 18 | 65574.2 KiB, 9 | 1037.1 KiB, 9 | 2045.0 KiB, 98 | 65574.1 KiB, 3 |
| lcet10.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 944.1 KiB, 97 | 290.1 KiB, 3 |
| lcet10.txt | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 944.1 KiB, 97 | 8226.1 KiB, 3 |
| lcet10.txt | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 944.2 KiB, 97 | 65574.1 KiB, 3 |
| plrabn12.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 998.0 KiB, 97 | 290.1 KiB, 3 |
| plrabn12.txt | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 998.0 KiB, 97 | 8226.1 KiB, 3 |
| plrabn12.txt | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 997.9 KiB, 97 | 65574.1 KiB, 3 |
| ptt5 | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1028.6 KiB, 97 | 290.1 KiB, 3 |
| ptt5 | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 1028.6 KiB, 97 | 8226.1 KiB, 3 |
| ptt5 | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 1028.6 KiB, 97 | 65574.1 KiB, 3 |
| sum | 0 | 79.9 KiB, 13 | 290.8 KiB, 9 | 77.1 KiB, 5 | 116.7 KiB, 94 | 290.1 KiB, 3 |
| sum | 6 | 79.9 KiB, 14 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 116.7 KiB, 94 | 8226.1 KiB, 3 |
| sum | 9e | 79.9 KiB, 14 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 116.7 KiB, 94 | 65574.1 KiB, 3 |
| xargs.1 | 0 | 23.9 KiB, 10 | 290.8 KiB, 9 | 77.1 KiB, 5 | 27.5 KiB, 91 | 290.1 KiB, 3 |
| xargs.1 | 6 | 23.9 KiB, 11 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 27.5 KiB, 91 | 8226.1 KiB, 3 |
| xargs.1 | 9e | 23.9 KiB, 11 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 27.5 KiB, 91 | 65574.1 KiB, 3 |
| E.coli | 0 | 271.9 KiB, 67 | 290.8 KiB, 13 | 269.2 KiB, 28 | 2063.5 KiB, 478 | 290.1 KiB, 3 |
| E.coli | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 12737.4 KiB, 101 | 8226.1 KiB, 3 |
| E.coli | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 12737.3 KiB, 101 | 65574.1 KiB, 3 |
| bible.txt | 0 | 271.9 KiB, 54 | 290.8 KiB, 12 | 269.1 KiB, 22 | 2063.4 KiB, 383 | 290.1 KiB, 3 |
| bible.txt | 6 | 4111.9 KiB, 20 | 8226.8 KiB, 9 | 4109.1 KiB, 11 | 8063.9 KiB, 100 | 8226.1 KiB, 3 |
| bible.txt | 9e | 4111.9 KiB, 20 | 65574.2 KiB, 9 | 4109.1 KiB, 11 | 8063.9 KiB, 100 | 65574.1 KiB, 3 |
| world192.txt | 0 | 271.9 KiB, 41 | 290.8 KiB, 11 | 269.1 KiB, 17 | 2063.4 KiB, 287 | 290.1 KiB, 3 |
| world192.txt | 6 | 4111.9 KiB, 20 | 8226.8 KiB, 9 | 4109.1 KiB, 11 | 6526.8 KiB, 100 | 8226.1 KiB, 3 |
| world192.txt | 9e | 4111.9 KiB, 20 | 65574.2 KiB, 9 | 4109.1 KiB, 11 | 6526.7 KiB, 100 | 65574.1 KiB, 3 |

#### Rust's executables

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rustc | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1670.5 KiB, 98 | 290.1 KiB, 3 |
| rustc | 6 | 1039.9 KiB, 18 | 8226.8 KiB, 9 | 1037.1 KiB, 9 | 1670.5 KiB, 98 | 8226.1 KiB, 3 |
| rustc | 9e | 1039.9 KiB, 18 | 65574.2 KiB, 9 | 1037.1 KiB, 9 | 1670.5 KiB, 98 | 65574.1 KiB, 3 |
| librustc_driver.so | 0 | 271.9 KiB, 1965 | 290.8 KiB, 159 | 273.1 KiB, 763 | 2067.4 KiB, 14355 | 290.1 KiB, 3 |
| librustc_driver.so | 6 | 8207.9 KiB, 135 | 8226.8 KiB, 15 | 8205.2 KiB, 73 | 57363.5 KiB, 703 | 8226.1 KiB, 3 |
| librustc_driver.so | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 416343.3 KiB, 107 | 65574.1 KiB, 3 |
| cargo | 0 | 271.9 KiB, 535 | 290.8 KiB, 49 | 270.1 KiB, 211 | 2064.4 KiB, 3904 | 290.1 KiB, 3 |
| cargo | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57363.3 KiB, 205 | 8226.1 KiB, 3 |
| cargo | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 107399.3 KiB, 106 | 65574.1 KiB, 3 |

Ours relative to liblzma's peak: 0.94x at the median, and at most 1.00x.

### Intel Xeon 6975P-C (1 vCPU under EC2 Nitro), in pieces of 4 KiB

Measured 2026-11-21, at `b66411c`, `decode-xz` 0.0.4.

The size of each decoder's value: ours 336 B, liblzma 136 B, lzma-rust2
4440 B, lzma-rs 0 B, xz4rust 8 B.

#### Synthetic

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| random | 0 | 261.4 KiB, 98 | 290.8 KiB, 16 | 257.2 KiB, 35 | 2925.7 KiB, 805 | 290.1 KiB, 3 |
| random | 6 | 8197.4 KiB, 20 | 8226.8 KiB, 9 | 8193.1 KiB, 11 | 23349.0 KiB, 228 | 8226.1 KiB, 3 |
| random | 9e | 16393.4 KiB, 21 | 65574.2 KiB, 9 | 8193.1 KiB, 11 | 23347.5 KiB, 228 | 65574.1 KiB, 3 |
| zeros | 0 | 271.9 KiB, 106 | 290.8 KiB, 16 | 269.2 KiB, 43 | 2067.6 KiB, 765 | 290.1 KiB, 3 |
| zeros | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 16403.5 KiB, 102 | 8226.1 KiB, 3 |
| zeros | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 16403.4 KiB, 102 | 65574.1 KiB, 3 |
| pattern | 0 | 271.9 KiB, 106 | 290.8 KiB, 16 | 269.2 KiB, 43 | 2067.6 KiB, 765 | 290.1 KiB, 3 |
| pattern | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 16403.4 KiB, 102 | 8226.1 KiB, 3 |
| pattern | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 16403.4 KiB, 102 | 65574.1 KiB, 3 |
| far | 0 | 261.4 KiB, 98 | 290.8 KiB, 16 | 257.2 KiB, 35 | 2925.6 KiB, 805 | 290.1 KiB, 3 |
| far | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 23357.9 KiB, 159 | 8226.1 KiB, 3 |
| far | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 23357.9 KiB, 159 | 65574.1 KiB, 3 |

#### Silesia

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| dickens | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2067.5 KiB, 956 | 290.1 KiB, 3 |
| dickens | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26357.0 KiB, 103 | 8226.1 KiB, 3 |
| dickens | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26357.0 KiB, 103 | 65574.1 KiB, 3 |
| mozilla | 0 | 271.9 KiB, 639 | 290.8 KiB, 57 | 270.1 KiB, 251 | 2949.7 KiB, 4654 | 290.1 KiB, 3 |
| mozilla | 6 | 8207.9 KiB, 56 | 8226.8 KiB, 11 | 8205.1 KiB, 29 | 57367.5 KiB, 300 | 8226.1 KiB, 3 |
| mozilla | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 115583.4 KiB, 106 | 65574.1 KiB, 3 |
| mr | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2067.5 KiB, 956 | 290.1 KiB, 3 |
| mr | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26140.3 KiB, 103 | 8226.1 KiB, 3 |
| mr | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26140.3 KiB, 103 | 65574.1 KiB, 3 |
| nci | 0 | 271.9 KiB, 418 | 290.8 KiB, 40 | 269.6 KiB, 165 | 2067.9 KiB, 3047 | 290.1 KiB, 3 |
| nci | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57367.4 KiB, 202 | 8226.1 KiB, 3 |
| nci | 9e | 32787.9 KiB, 23 | 65574.2 KiB, 9 | 32785.1 KiB, 14 | 65558.4 KiB, 104 | 65574.1 KiB, 3 |
| ooffice | 0 | 271.9 KiB, 80 | 290.8 KiB, 14 | 269.2 KiB, 33 | 2067.5 KiB, 575 | 290.1 KiB, 3 |
| ooffice | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 14219.4 KiB, 102 | 8226.1 KiB, 3 |
| ooffice | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 14219.4 KiB, 102 | 65574.1 KiB, 3 |
| osdb | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2067.5 KiB, 956 | 290.1 KiB, 3 |
| osdb | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26252.7 KiB, 103 | 8226.1 KiB, 3 |
| osdb | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26252.7 KiB, 103 | 65574.1 KiB, 3 |
| reymont | 0 | 271.9 KiB, 93 | 290.8 KiB, 15 | 269.2 KiB, 38 | 2067.6 KiB, 669 | 290.1 KiB, 3 |
| reymont | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 14683.3 KiB, 102 | 8226.1 KiB, 3 |
| reymont | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 14683.3 KiB, 102 | 65574.1 KiB, 3 |
| samba | 0 | 271.9 KiB, 275 | 290.8 KiB, 29 | 269.6 KiB, 110 | 2068.0 KiB, 2002 | 290.1 KiB, 3 |
| samba | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 53891.4 KiB, 104 | 8226.1 KiB, 3 |
| samba | 9e | 32787.9 KiB, 23 | 65574.2 KiB, 9 | 32785.1 KiB, 14 | 53891.4 KiB, 104 | 65574.1 KiB, 3 |
| sao | 0 | 271.9 KiB, 93 | 290.8 KiB, 15 | 269.2 KiB, 38 | 2067.5 KiB, 670 | 290.1 KiB, 3 |
| sao | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 15293.4 KiB, 102 | 8226.1 KiB, 3 |
| sao | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 15293.4 KiB, 102 | 65574.1 KiB, 3 |
| webster | 0 | 271.9 KiB, 522 | 290.8 KiB, 48 | 270.1 KiB, 206 | 2068.5 KiB, 3808 | 290.1 KiB, 3 |
| webster | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57367.4 KiB, 203 | 8226.1 KiB, 3 |
| webster | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 106047.3 KiB, 105 | 65574.1 KiB, 3 |
| xml | 0 | 271.9 KiB, 79 | 290.8 KiB, 14 | 269.2 KiB, 32 | 2067.5 KiB, 572 | 290.1 KiB, 3 |
| xml | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 13431.4 KiB, 102 | 8226.1 KiB, 3 |
| xml | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 13431.4 KiB, 102 | 65574.1 KiB, 3 |
| x-ray | 0 | 271.9 KiB, 118 | 290.8 KiB, 17 | 269.2 KiB, 48 | 2067.6 KiB, 858 | 290.1 KiB, 3 |
| x-ray | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 24679.0 KiB, 103 | 8226.1 KiB, 3 |
| x-ray | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 24679.0 KiB, 103 | 65574.1 KiB, 3 |

#### Large files

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 0 | 271.9 KiB, 1250 | 290.8 KiB, 104 | 271.1 KiB, 487 | 2069.4 KiB, 9128 | 290.1 KiB, 3 |
| enwik8 | 6 | 8207.9 KiB, 78 | 8226.8 KiB, 12 | 8205.1 KiB, 42 | 57367.4 KiB, 404 | 8226.1 KiB, 3 |
| enwik8 | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 228755.4 KiB, 106 | 65574.1 KiB, 3 |
| linux-6.12.tar | 0 | 271.9 KiB, 1250 | 290.8 KiB, 104 | 271.1 KiB, 487 | 2069.5 KiB, 9128 | 290.1 KiB, 3 |
| linux-6.12.tar | 6 | 8207.9 KiB, 78 | 8226.8 KiB, 12 | 8205.1 KiB, 42 | 57363.5 KiB, 404 | 8226.1 KiB, 3 |
| linux-6.12.tar | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 228755.4 KiB, 106 | 65574.1 KiB, 3 |

#### Canterbury

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 424.0 KiB, 97 | 290.1 KiB, 3 |
| alice29.txt | 6 | 271.9 KiB, 16 | 8226.8 KiB, 9 | 269.1 KiB, 7 | 424.0 KiB, 97 | 8226.1 KiB, 3 |
| alice29.txt | 9e | 271.9 KiB, 16 | 65574.2 KiB, 9 | 269.1 KiB, 7 | 423.9 KiB, 97 | 65574.1 KiB, 3 |
| asyoulik.txt | 0 | 143.9 KiB, 14 | 290.8 KiB, 9 | 141.1 KiB, 6 | 269.7 KiB, 96 | 290.1 KiB, 3 |
| asyoulik.txt | 6 | 143.9 KiB, 15 | 8226.8 KiB, 9 | 141.1 KiB, 6 | 269.7 KiB, 96 | 8226.1 KiB, 3 |
| asyoulik.txt | 9e | 143.9 KiB, 15 | 65574.2 KiB, 9 | 141.1 KiB, 6 | 269.7 KiB, 96 | 65574.1 KiB, 3 |
| cp.html | 0 | 47.9 KiB, 12 | 290.8 KiB, 9 | 77.1 KiB, 5 | 75.5 KiB, 94 | 290.1 KiB, 3 |
| cp.html | 6 | 47.9 KiB, 13 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 75.5 KiB, 94 | 8226.1 KiB, 3 |
| cp.html | 9e | 47.9 KiB, 13 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 75.4 KiB, 94 | 65574.1 KiB, 3 |
| fields.c | 0 | 31.9 KiB, 11 | 290.8 KiB, 9 | 77.1 KiB, 5 | 46.3 KiB, 93 | 290.1 KiB, 3 |
| fields.c | 6 | 31.9 KiB, 12 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 46.3 KiB, 93 | 8226.1 KiB, 3 |
| fields.c | 9e | 31.9 KiB, 12 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 46.3 KiB, 93 | 65574.1 KiB, 3 |
| grammar.lsp | 0 | 19.9 KiB, 9 | 290.8 KiB, 9 | 77.1 KiB, 5 | 27.0 KiB, 91 | 290.1 KiB, 3 |
| grammar.lsp | 6 | 19.9 KiB, 10 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 27.1 KiB, 91 | 8226.1 KiB, 3 |
| grammar.lsp | 9e | 19.9 KiB, 10 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 27.1 KiB, 91 | 65574.1 KiB, 3 |
| kennedy.xls | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 2049.0 KiB, 99 | 290.1 KiB, 3 |
| kennedy.xls | 6 | 1039.9 KiB, 18 | 8226.8 KiB, 9 | 1037.1 KiB, 9 | 2049.0 KiB, 99 | 8226.1 KiB, 3 |
| kennedy.xls | 9e | 1039.9 KiB, 18 | 65574.2 KiB, 9 | 1037.1 KiB, 9 | 2049.0 KiB, 99 | 65574.1 KiB, 3 |
| lcet10.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 948.1 KiB, 98 | 290.1 KiB, 3 |
| lcet10.txt | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 948.1 KiB, 98 | 8226.1 KiB, 3 |
| lcet10.txt | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 948.1 KiB, 98 | 65574.1 KiB, 3 |
| plrabn12.txt | 0 | 272.0 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1002.0 KiB, 98 | 290.1 KiB, 3 |
| plrabn12.txt | 6 | 528.0 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 1002.0 KiB, 98 | 8226.1 KiB, 3 |
| plrabn12.txt | 9e | 528.0 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 1002.0 KiB, 98 | 65574.1 KiB, 3 |
| ptt5 | 0 | 272.0 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1032.6 KiB, 98 | 290.1 KiB, 3 |
| ptt5 | 6 | 528.0 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 1032.6 KiB, 98 | 8226.1 KiB, 3 |
| ptt5 | 9e | 528.0 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 1032.6 KiB, 98 | 65574.1 KiB, 3 |
| sum | 0 | 80.0 KiB, 13 | 290.8 KiB, 9 | 77.1 KiB, 5 | 120.8 KiB, 95 | 290.1 KiB, 3 |
| sum | 6 | 80.0 KiB, 14 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 120.7 KiB, 95 | 8226.1 KiB, 3 |
| sum | 9e | 79.9 KiB, 14 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 120.7 KiB, 95 | 65574.1 KiB, 3 |
| xargs.1 | 0 | 23.9 KiB, 10 | 290.8 KiB, 9 | 77.1 KiB, 5 | 31.5 KiB, 92 | 290.1 KiB, 3 |
| xargs.1 | 6 | 23.9 KiB, 11 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 31.5 KiB, 92 | 8226.1 KiB, 3 |
| xargs.1 | 9e | 23.9 KiB, 11 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 31.5 KiB, 92 | 65574.1 KiB, 3 |
| E.coli | 0 | 271.9 KiB, 67 | 290.8 KiB, 13 | 269.2 KiB, 28 | 2067.5 KiB, 479 | 290.1 KiB, 3 |
| E.coli | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 12741.4 KiB, 102 | 8226.1 KiB, 3 |
| E.coli | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 12741.4 KiB, 102 | 65574.1 KiB, 3 |
| bible.txt | 0 | 271.9 KiB, 54 | 290.8 KiB, 12 | 269.1 KiB, 22 | 2067.5 KiB, 384 | 290.1 KiB, 3 |
| bible.txt | 6 | 4111.9 KiB, 20 | 8226.8 KiB, 9 | 4109.1 KiB, 11 | 8067.9 KiB, 101 | 8226.1 KiB, 3 |
| bible.txt | 9e | 4111.9 KiB, 20 | 65574.2 KiB, 9 | 4109.1 KiB, 11 | 8067.9 KiB, 101 | 65574.1 KiB, 3 |
| world192.txt | 0 | 271.9 KiB, 41 | 290.8 KiB, 11 | 269.1 KiB, 17 | 2067.5 KiB, 288 | 290.1 KiB, 3 |
| world192.txt | 6 | 4111.9 KiB, 20 | 8226.8 KiB, 9 | 4109.1 KiB, 11 | 6530.9 KiB, 101 | 8226.1 KiB, 3 |
| world192.txt | 9e | 4111.9 KiB, 20 | 65574.2 KiB, 9 | 4109.1 KiB, 11 | 6530.9 KiB, 101 | 65574.1 KiB, 3 |

#### Rust's executables

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rustc | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1674.6 KiB, 99 | 290.1 KiB, 3 |
| rustc | 6 | 1039.9 KiB, 18 | 8226.8 KiB, 9 | 1037.1 KiB, 9 | 1674.6 KiB, 99 | 8226.1 KiB, 3 |
| rustc | 9e | 1039.9 KiB, 18 | 65574.2 KiB, 9 | 1037.1 KiB, 9 | 1674.6 KiB, 99 | 65574.1 KiB, 3 |
| librustc_driver.so | 0 | 271.9 KiB, 1965 | 290.8 KiB, 159 | 273.1 KiB, 763 | 2071.5 KiB, 14356 | 290.1 KiB, 3 |
| librustc_driver.so | 6 | 8207.9 KiB, 135 | 8226.8 KiB, 15 | 8205.2 KiB, 73 | 57367.5 KiB, 704 | 8226.1 KiB, 3 |
| librustc_driver.so | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 416347.3 KiB, 108 | 65574.1 KiB, 3 |
| cargo | 0 | 271.9 KiB, 535 | 290.8 KiB, 49 | 270.1 KiB, 211 | 2068.5 KiB, 3905 | 290.1 KiB, 3 |
| cargo | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57367.4 KiB, 206 | 8226.1 KiB, 3 |
| cargo | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 107403.4 KiB, 107 | 65574.1 KiB, 3 |

Ours relative to liblzma's peak: 0.94x at the median, and at most 1.00x.

### Intel Xeon 6975P-C (1 vCPU under EC2 Nitro), in pieces of 64 KiB

Measured 2026-11-21, at `b66411c`, `decode-xz` 0.0.4.

The size of each decoder's value: ours 336 B, liblzma 136 B, lzma-rust2
4440 B, lzma-rs 0 B, xz4rust 8 B.

#### Synthetic

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| random | 0 | 261.4 KiB, 98 | 290.8 KiB, 16 | 257.2 KiB, 35 | 2985.7 KiB, 805 | 290.1 KiB, 3 |
| random | 6 | 8197.4 KiB, 20 | 8226.8 KiB, 9 | 8193.1 KiB, 11 | 23409.0 KiB, 228 | 8226.1 KiB, 3 |
| random | 9e | 16393.4 KiB, 21 | 65574.2 KiB, 9 | 8193.1 KiB, 11 | 23407.5 KiB, 228 | 65574.1 KiB, 3 |
| zeros | 0 | 271.9 KiB, 106 | 290.8 KiB, 16 | 269.2 KiB, 43 | 2127.6 KiB, 765 | 290.1 KiB, 3 |
| zeros | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 16463.4 KiB, 102 | 8226.1 KiB, 3 |
| zeros | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 16463.4 KiB, 102 | 65574.1 KiB, 3 |
| pattern | 0 | 271.9 KiB, 106 | 290.8 KiB, 16 | 269.2 KiB, 43 | 2127.6 KiB, 765 | 290.1 KiB, 3 |
| pattern | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 16463.4 KiB, 102 | 8226.1 KiB, 3 |
| pattern | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 16463.4 KiB, 102 | 65574.1 KiB, 3 |
| far | 0 | 261.4 KiB, 98 | 290.8 KiB, 16 | 257.2 KiB, 35 | 2985.6 KiB, 805 | 290.1 KiB, 3 |
| far | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 23417.9 KiB, 159 | 8226.1 KiB, 3 |
| far | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 23417.9 KiB, 159 | 65574.1 KiB, 3 |

#### Silesia

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| dickens | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2127.6 KiB, 956 | 290.1 KiB, 3 |
| dickens | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26417.0 KiB, 103 | 8226.1 KiB, 3 |
| dickens | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26416.9 KiB, 103 | 65574.1 KiB, 3 |
| mozilla | 0 | 271.9 KiB, 639 | 290.8 KiB, 57 | 270.1 KiB, 251 | 3009.7 KiB, 4654 | 290.1 KiB, 3 |
| mozilla | 6 | 8207.9 KiB, 56 | 8226.8 KiB, 11 | 8205.1 KiB, 29 | 57427.5 KiB, 300 | 8226.1 KiB, 3 |
| mozilla | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 115643.4 KiB, 106 | 65574.1 KiB, 3 |
| mr | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2127.6 KiB, 956 | 290.1 KiB, 3 |
| mr | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26200.3 KiB, 103 | 8226.1 KiB, 3 |
| mr | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26200.3 KiB, 103 | 65574.1 KiB, 3 |
| nci | 0 | 271.9 KiB, 418 | 290.8 KiB, 40 | 269.6 KiB, 165 | 2128.0 KiB, 3047 | 290.1 KiB, 3 |
| nci | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57427.4 KiB, 202 | 8226.1 KiB, 3 |
| nci | 9e | 32787.9 KiB, 23 | 65574.2 KiB, 9 | 32785.1 KiB, 14 | 65618.5 KiB, 104 | 65574.1 KiB, 3 |
| ooffice | 0 | 271.9 KiB, 80 | 290.8 KiB, 14 | 269.2 KiB, 33 | 2127.5 KiB, 575 | 290.1 KiB, 3 |
| ooffice | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 14279.4 KiB, 102 | 8226.1 KiB, 3 |
| ooffice | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 14279.4 KiB, 102 | 65574.1 KiB, 3 |
| osdb | 0 | 271.9 KiB, 132 | 290.8 KiB, 18 | 269.3 KiB, 54 | 2127.6 KiB, 956 | 290.1 KiB, 3 |
| osdb | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 26312.7 KiB, 103 | 8226.1 KiB, 3 |
| osdb | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 26312.8 KiB, 103 | 65574.1 KiB, 3 |
| reymont | 0 | 271.9 KiB, 93 | 290.8 KiB, 15 | 269.2 KiB, 38 | 2127.6 KiB, 669 | 290.1 KiB, 3 |
| reymont | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 14743.4 KiB, 102 | 8226.1 KiB, 3 |
| reymont | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 14743.3 KiB, 102 | 65574.1 KiB, 3 |
| samba | 0 | 271.9 KiB, 275 | 290.8 KiB, 29 | 269.6 KiB, 110 | 2128.0 KiB, 2002 | 290.1 KiB, 3 |
| samba | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 53951.4 KiB, 104 | 8226.1 KiB, 3 |
| samba | 9e | 32787.9 KiB, 23 | 65574.2 KiB, 9 | 32785.1 KiB, 14 | 53951.4 KiB, 104 | 65574.1 KiB, 3 |
| sao | 0 | 271.9 KiB, 93 | 290.8 KiB, 15 | 269.2 KiB, 38 | 2127.6 KiB, 670 | 290.1 KiB, 3 |
| sao | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 15353.4 KiB, 102 | 8226.1 KiB, 3 |
| sao | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 15353.4 KiB, 102 | 65574.1 KiB, 3 |
| webster | 0 | 271.9 KiB, 522 | 290.8 KiB, 48 | 270.1 KiB, 206 | 2128.5 KiB, 3808 | 290.1 KiB, 3 |
| webster | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57427.4 KiB, 203 | 8226.1 KiB, 3 |
| webster | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 106107.4 KiB, 105 | 65574.1 KiB, 3 |
| xml | 0 | 271.9 KiB, 79 | 290.8 KiB, 14 | 269.2 KiB, 32 | 2127.5 KiB, 572 | 290.1 KiB, 3 |
| xml | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 13491.4 KiB, 102 | 8226.1 KiB, 3 |
| xml | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 13491.4 KiB, 102 | 65574.1 KiB, 3 |
| x-ray | 0 | 271.9 KiB, 118 | 290.8 KiB, 17 | 269.2 KiB, 48 | 2127.6 KiB, 858 | 290.1 KiB, 3 |
| x-ray | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 24739.1 KiB, 103 | 8226.1 KiB, 3 |
| x-ray | 9e | 16399.9 KiB, 22 | 65574.2 KiB, 9 | 16397.1 KiB, 13 | 24739.1 KiB, 103 | 65574.1 KiB, 3 |

#### Large files

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 0 | 271.9 KiB, 1250 | 290.8 KiB, 104 | 271.1 KiB, 487 | 2129.4 KiB, 9128 | 290.1 KiB, 3 |
| enwik8 | 6 | 8207.9 KiB, 78 | 8226.8 KiB, 12 | 8205.1 KiB, 42 | 57427.4 KiB, 404 | 8226.1 KiB, 3 |
| enwik8 | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 228815.3 KiB, 106 | 65574.1 KiB, 3 |
| linux-6.12.tar | 0 | 271.9 KiB, 1250 | 290.8 KiB, 104 | 271.1 KiB, 487 | 2129.4 KiB, 9128 | 290.1 KiB, 3 |
| linux-6.12.tar | 6 | 8207.9 KiB, 78 | 8226.8 KiB, 12 | 8205.1 KiB, 42 | 57423.4 KiB, 404 | 8226.1 KiB, 3 |
| linux-6.12.tar | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 228815.3 KiB, 106 | 65574.1 KiB, 3 |

#### Canterbury

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 483.9 KiB, 97 | 290.1 KiB, 3 |
| alice29.txt | 6 | 271.9 KiB, 16 | 8226.8 KiB, 9 | 269.1 KiB, 7 | 484.0 KiB, 97 | 8226.1 KiB, 3 |
| alice29.txt | 9e | 271.9 KiB, 16 | 65574.2 KiB, 9 | 269.1 KiB, 7 | 484.0 KiB, 97 | 65574.1 KiB, 3 |
| asyoulik.txt | 0 | 143.9 KiB, 14 | 290.8 KiB, 9 | 141.1 KiB, 6 | 329.7 KiB, 96 | 290.1 KiB, 3 |
| asyoulik.txt | 6 | 143.9 KiB, 15 | 8226.8 KiB, 9 | 141.1 KiB, 6 | 329.7 KiB, 96 | 8226.1 KiB, 3 |
| asyoulik.txt | 9e | 143.9 KiB, 15 | 65574.2 KiB, 9 | 141.1 KiB, 6 | 329.7 KiB, 96 | 65574.1 KiB, 3 |
| cp.html | 0 | 47.9 KiB, 12 | 290.8 KiB, 9 | 77.1 KiB, 5 | 135.4 KiB, 94 | 290.1 KiB, 3 |
| cp.html | 6 | 47.9 KiB, 13 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 135.5 KiB, 94 | 8226.1 KiB, 3 |
| cp.html | 9e | 47.9 KiB, 13 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 135.4 KiB, 94 | 65574.1 KiB, 3 |
| fields.c | 0 | 31.9 KiB, 11 | 290.8 KiB, 9 | 77.1 KiB, 5 | 106.3 KiB, 93 | 290.1 KiB, 3 |
| fields.c | 6 | 31.9 KiB, 12 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 106.3 KiB, 93 | 8226.1 KiB, 3 |
| fields.c | 9e | 31.9 KiB, 12 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 106.3 KiB, 93 | 65574.1 KiB, 3 |
| grammar.lsp | 0 | 19.9 KiB, 9 | 290.8 KiB, 9 | 77.1 KiB, 5 | 87.0 KiB, 91 | 290.1 KiB, 3 |
| grammar.lsp | 6 | 19.9 KiB, 10 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 87.0 KiB, 91 | 8226.1 KiB, 3 |
| grammar.lsp | 9e | 19.9 KiB, 10 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 87.0 KiB, 91 | 65574.1 KiB, 3 |
| kennedy.xls | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 2109.0 KiB, 99 | 290.1 KiB, 3 |
| kennedy.xls | 6 | 1039.9 KiB, 18 | 8226.8 KiB, 9 | 1037.1 KiB, 9 | 2109.0 KiB, 99 | 8226.1 KiB, 3 |
| kennedy.xls | 9e | 1039.9 KiB, 18 | 65574.2 KiB, 9 | 1037.1 KiB, 9 | 2109.0 KiB, 99 | 65574.1 KiB, 3 |
| lcet10.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1008.1 KiB, 98 | 290.1 KiB, 3 |
| lcet10.txt | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 1008.1 KiB, 98 | 8226.1 KiB, 3 |
| lcet10.txt | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 1008.2 KiB, 98 | 65574.1 KiB, 3 |
| plrabn12.txt | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1062.0 KiB, 98 | 290.1 KiB, 3 |
| plrabn12.txt | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 1062.0 KiB, 98 | 8226.1 KiB, 3 |
| plrabn12.txt | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 1061.9 KiB, 98 | 65574.1 KiB, 3 |
| ptt5 | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1092.6 KiB, 98 | 290.1 KiB, 3 |
| ptt5 | 6 | 527.9 KiB, 17 | 8226.8 KiB, 9 | 525.1 KiB, 8 | 1092.6 KiB, 98 | 8226.1 KiB, 3 |
| ptt5 | 9e | 527.9 KiB, 17 | 65574.2 KiB, 9 | 525.1 KiB, 8 | 1092.6 KiB, 98 | 65574.1 KiB, 3 |
| sum | 0 | 79.9 KiB, 13 | 290.8 KiB, 9 | 77.1 KiB, 5 | 180.7 KiB, 95 | 290.1 KiB, 3 |
| sum | 6 | 79.9 KiB, 14 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 180.7 KiB, 95 | 8226.1 KiB, 3 |
| sum | 9e | 79.9 KiB, 14 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 180.7 KiB, 95 | 65574.1 KiB, 3 |
| xargs.1 | 0 | 23.9 KiB, 10 | 290.8 KiB, 9 | 77.1 KiB, 5 | 91.5 KiB, 92 | 290.1 KiB, 3 |
| xargs.1 | 6 | 23.9 KiB, 11 | 8226.8 KiB, 9 | 77.1 KiB, 5 | 91.5 KiB, 92 | 8226.1 KiB, 3 |
| xargs.1 | 9e | 23.9 KiB, 11 | 65574.2 KiB, 9 | 77.1 KiB, 5 | 91.5 KiB, 92 | 65574.1 KiB, 3 |
| E.coli | 0 | 271.9 KiB, 67 | 290.8 KiB, 13 | 269.2 KiB, 28 | 2127.5 KiB, 479 | 290.1 KiB, 3 |
| E.coli | 6 | 8207.9 KiB, 21 | 8226.8 KiB, 9 | 8205.1 KiB, 12 | 12801.4 KiB, 102 | 8226.1 KiB, 3 |
| E.coli | 9e | 8207.9 KiB, 21 | 65574.2 KiB, 9 | 8205.1 KiB, 12 | 12801.3 KiB, 102 | 65574.1 KiB, 3 |
| bible.txt | 0 | 271.9 KiB, 54 | 290.8 KiB, 12 | 269.1 KiB, 22 | 2127.4 KiB, 384 | 290.1 KiB, 3 |
| bible.txt | 6 | 4111.9 KiB, 20 | 8226.8 KiB, 9 | 4109.1 KiB, 11 | 8127.9 KiB, 101 | 8226.1 KiB, 3 |
| bible.txt | 9e | 4111.9 KiB, 20 | 65574.2 KiB, 9 | 4109.1 KiB, 11 | 8127.9 KiB, 101 | 65574.1 KiB, 3 |
| world192.txt | 0 | 271.9 KiB, 41 | 290.8 KiB, 11 | 269.1 KiB, 17 | 2127.4 KiB, 288 | 290.1 KiB, 3 |
| world192.txt | 6 | 4111.9 KiB, 20 | 8226.8 KiB, 9 | 4109.1 KiB, 11 | 6590.8 KiB, 101 | 8226.1 KiB, 3 |
| world192.txt | 9e | 4111.9 KiB, 20 | 65574.2 KiB, 9 | 4109.1 KiB, 11 | 6590.7 KiB, 101 | 65574.1 KiB, 3 |

#### Rust's executables

| Input | Level | ours | liblzma | lzma-rust2 | lzma-rs | xz4rust |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rustc | 0 | 271.9 KiB, 15 | 290.8 KiB, 9 | 269.1 KiB, 7 | 1734.6 KiB, 99 | 290.1 KiB, 3 |
| rustc | 6 | 1039.9 KiB, 18 | 8226.8 KiB, 9 | 1037.1 KiB, 9 | 1734.6 KiB, 99 | 8226.1 KiB, 3 |
| rustc | 9e | 1039.9 KiB, 18 | 65574.2 KiB, 9 | 1037.1 KiB, 9 | 1734.6 KiB, 99 | 65574.1 KiB, 3 |
| librustc_driver.so | 0 | 271.9 KiB, 1965 | 290.8 KiB, 159 | 273.1 KiB, 763 | 2131.4 KiB, 14356 | 290.1 KiB, 3 |
| librustc_driver.so | 6 | 8207.9 KiB, 135 | 8226.8 KiB, 15 | 8205.2 KiB, 73 | 57427.5 KiB, 704 | 8226.1 KiB, 3 |
| librustc_driver.so | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 416407.3 KiB, 108 | 65574.1 KiB, 3 |
| cargo | 0 | 271.9 KiB, 535 | 290.8 KiB, 49 | 270.1 KiB, 211 | 2128.4 KiB, 3905 | 290.1 KiB, 3 |
| cargo | 6 | 8207.9 KiB, 40 | 8226.8 KiB, 10 | 8205.1 KiB, 22 | 57427.4 KiB, 206 | 8226.1 KiB, 3 |
| cargo | 9e | 65555.9 KiB, 24 | 65574.2 KiB, 9 | 65553.1 KiB, 15 | 107463.3 KiB, 107 | 65574.1 KiB, 3 |

Ours relative to liblzma's peak: 0.94x at the median, and at most 1.00x.

<!-- filer-bench: end -->

## Checkpoints

The time to take a checkpoint and to resume from one, and the bytes each
holds, in streams decoded in pieces of 32 KiB, per machine: a row per
input and level, with our decoder's µs to take one and to resume from
one, each the median over several processes of the median over the
places they were taken at, and the most heap one held and its
allocations. No counterpart offers them. It offers them between
blocks, so a stream of one block has none.

<!-- filer-bench: checkpoints -->

<!-- filer-bench: end -->

## Code size

The bytes of `.text` the decoder adds to a minimal `#![no_std]` program
that decodes a stream with it, from construction to `finish`, built
portable with fat LTO; read-only data, such as tables, is not counted.

<!-- filer-bench: size -->

| Target | `opt-level` 3 | `opt-level` "s" |
| --- | ---: | ---: |
| aarch64-apple-darwin | 30512 | 25132 |
| aarch64-unknown-linux-gnu | 30020 | 24776 |
| x86_64-unknown-linux-gnu | 38634 | 29431 |

On aarch64-apple-darwin, measured 2026-11-07, at `86d2427`, `decode-xz`
0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On aarch64-unknown-linux-gnu, measured 2026-11-16, at `d73cac4`,
`decode-xz` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On x86_64-unknown-linux-gnu, measured 2026-11-16, at `d73cac4`,
`decode-xz` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

<!-- filer-bench: end -->
