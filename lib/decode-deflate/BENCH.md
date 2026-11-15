# decode-deflate benchmarks

The figures of `decode-deflate`'s decoder, alone and against the main
alternatives, measured by the protocol of this repository's
[`bench/METHOD.md`](../../bench/METHOD.md): the same inputs, machines,
and builds for every decoder, each figure a median with its 95%
confidence interval, and every output checked against the original.

The harness's `tables` command generates the figures below from its raw
data, which stays on the machines that measured it; small streams,
checkpoints, and Deflate64 are not measured yet.

## Counterparts

| Decoder | Crate | Language |
| --- | --- | --- |
| zlib, the reference | `libz-sys` | C |
| zlib-ng | `libz-ng-sys` | C |
| libdeflate | `libdeflater` | C, whole buffers only |
| zlib-rs | `zlib-rs` | Rust |
| miniz_oxide | `miniz_oxide` | Rust |
| zune-inflate | `zune-inflate` | Rust, whole buffers only |
| yazi | `yazi` | Rust |

## Inputs

The protocol's corpus and synthetic cases, encoded by zlib at levels 1,
6, and 9, and libdeflate at 12, as raw Deflate.

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

Not measured yet.

<!-- filer-bench: end -->

## Streaming

MB/s with input and output in pieces of 4 KiB and of 64 KiB.

<!-- filer-bench: streaming -->

Not measured yet.

<!-- filer-bench: end -->

## Small streams

The time per stream of about 64 B, 1 KiB, and 16 KiB packed, the
decoder's construction included.

## Memory

Peak heap and allocations per stream, and the size of the decoder
itself.

<!-- filer-bench: memory -->

### AMD EPYC 9R45 (1 vCPU under EC2 Nitro), whole

Measured 2026-11-15, at `9e96979`, `decode-deflate` 0.0.4.

The size of each decoder's value: ours 168 B, zlib 112 B, zlib-ng 104 B,
libdeflate 8 B, zlib-rs 128 B, miniz_oxide 8 B, zune-inflate 11520 B,
yazi 7296 B.

#### Synthetic

| Input | Level | ours | zlib | zlib-ng | libdeflate | zlib-rs | miniz_oxide | zune-inflate | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| random | 1 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16388.0 KiB, 2 | 0.0 KiB, 0 |
| random | 6 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| random | 9 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| random | 12 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| zeros | 1 | 147.5 KiB, 23 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| zeros | 6 | 147.5 KiB, 23 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| zeros | 9 | 147.5 KiB, 23 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| zeros | 12 | 147.5 KiB, 283 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| pattern | 1 | 147.5 KiB, 23 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| pattern | 6 | 147.5 KiB, 23 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| pattern | 9 | 147.5 KiB, 23 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| pattern | 12 | 147.5 KiB, 283 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| far | 1 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| far | 6 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| far | 9 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| far | 12 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |

#### Silesia

| Input | Level | ours | zlib | zlib-ng | libdeflate | zlib-rs | miniz_oxide | zune-inflate | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| dickens | 1 | 156.5 KiB, 1731 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19908.0 KiB, 2 | 0.0 KiB, 0 |
| dickens | 6 | 156.5 KiB, 1527 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19907.1 KiB, 2 | 0.0 KiB, 0 |
| dickens | 9 | 156.5 KiB, 1515 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19907.1 KiB, 2 | 0.0 KiB, 0 |
| dickens | 12 | 156.5 KiB, 639 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19907.1 KiB, 2 | 0.0 KiB, 0 |
| mozilla | 1 | 156.5 KiB, 10485 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 100044.0 KiB, 2 | 0.0 KiB, 0 |
| mozilla | 6 | 156.5 KiB, 9989 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 100044.0 KiB, 2 | 0.0 KiB, 0 |
| mozilla | 9 | 156.5 KiB, 9960 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 100044.0 KiB, 2 | 0.0 KiB, 0 |
| mozilla | 12 | 156.5 KiB, 23342 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 100044.0 KiB, 2 | 0.0 KiB, 0 |
| mr | 1 | 156.5 KiB, 1576 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19473.8 KiB, 2 | 0.0 KiB, 0 |
| mr | 6 | 156.5 KiB, 1662 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19473.8 KiB, 2 | 0.0 KiB, 0 |
| mr | 9 | 156.5 KiB, 1670 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19473.8 KiB, 2 | 0.0 KiB, 0 |
| mr | 12 | 156.5 KiB, 1362 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19473.8 KiB, 2 | 0.0 KiB, 0 |
| nci | 1 | 156.5 KiB, 1671 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 65536.0 KiB, 2 | 0.0 KiB, 0 |
| nci | 6 | 156.5 KiB, 1311 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 65536.0 KiB, 2 | 0.0 KiB, 0 |
| nci | 9 | 156.5 KiB, 1311 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 65536.0 KiB, 2 | 0.0 KiB, 0 |
| nci | 12 | 156.5 KiB, 3853 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 65536.0 KiB, 2 | 0.0 KiB, 0 |
| ooffice | 1 | 156.5 KiB, 1635 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12016.0 KiB, 2 | 0.0 KiB, 0 |
| ooffice | 6 | 156.5 KiB, 1605 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12016.0 KiB, 2 | 0.0 KiB, 0 |
| ooffice | 9 | 156.5 KiB, 1597 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12016.0 KiB, 2 | 0.0 KiB, 0 |
| ooffice | 12 | 156.5 KiB, 1260 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12016.0 KiB, 2 | 0.0 KiB, 0 |
| osdb | 1 | 156.4 KiB, 2115 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19698.6 KiB, 2 | 0.0 KiB, 0 |
| osdb | 6 | 156.4 KiB, 1987 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19698.6 KiB, 2 | 0.0 KiB, 0 |
| osdb | 9 | 156.4 KiB, 1982 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19698.6 KiB, 2 | 0.0 KiB, 0 |
| osdb | 12 | 156.4 KiB, 423 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19698.6 KiB, 2 | 0.0 KiB, 0 |
| reymont | 1 | 156.5 KiB, 915 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12943.8 KiB, 2 | 0.0 KiB, 0 |
| reymont | 6 | 156.5 KiB, 702 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12943.8 KiB, 2 | 0.0 KiB, 0 |
| reymont | 9 | 156.5 KiB, 699 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12943.8 KiB, 2 | 0.0 KiB, 0 |
| reymont | 12 | 155.5 KiB, 344 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12943.8 KiB, 2 | 0.0 KiB, 0 |
| samba | 1 | 156.5 KiB, 2859 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 42204.0 KiB, 2 | 0.0 KiB, 0 |
| samba | 6 | 156.5 KiB, 2629 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 42204.0 KiB, 2 | 0.0 KiB, 0 |
| samba | 9 | 156.5 KiB, 2612 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 42204.0 KiB, 2 | 0.0 KiB, 0 |
| samba | 12 | 156.5 KiB, 8936 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 42204.0 KiB, 2 | 0.0 KiB, 0 |
| sao | 1 | 156.2 KiB, 3016 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 14164.0 KiB, 2 | 0.0 KiB, 0 |
| sao | 6 | 156.2 KiB, 3030 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 14164.0 KiB, 2 | 0.0 KiB, 0 |
| sao | 9 | 156.2 KiB, 3041 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 14164.0 KiB, 2 | 0.0 KiB, 0 |
| sao | 12 | 156.2 KiB, 303 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 14164.0 KiB, 2 | 0.0 KiB, 0 |
| webster | 1 | 156.5 KiB, 5559 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 80976.0 KiB, 2 | 0.0 KiB, 0 |
| webster | 6 | 156.5 KiB, 4983 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 80976.0 KiB, 2 | 0.0 KiB, 0 |
| webster | 9 | 156.5 KiB, 4959 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 80976.0 KiB, 2 | 0.0 KiB, 0 |
| webster | 12 | 156.5 KiB, 1933 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 80976.0 KiB, 2 | 0.0 KiB, 0 |
| xml | 1 | 156.5 KiB, 375 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 10440.0 KiB, 2 | 0.0 KiB, 0 |
| xml | 6 | 156.5 KiB, 291 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 10440.0 KiB, 2 | 0.0 KiB, 0 |
| xml | 9 | 156.5 KiB, 291 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 10440.0 KiB, 2 | 0.0 KiB, 0 |
| xml | 12 | 156.5 KiB, 705 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 10440.0 KiB, 2 | 0.0 KiB, 0 |
| x-ray | 1 | 156.2 KiB, 2677 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16551.3 KiB, 2 | 0.0 KiB, 0 |
| x-ray | 6 | 156.2 KiB, 3081 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16551.3 KiB, 2 | 0.0 KiB, 0 |
| x-ray | 9 | 156.2 KiB, 3081 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16551.3 KiB, 2 | 0.0 KiB, 0 |
| x-ray | 12 | 156.2 KiB, 409 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16551.3 KiB, 2 | 0.0 KiB, 0 |

#### Large files

| Input | Level | ours | zlib | zlib-ng | libdeflate | zlib-rs | miniz_oxide | zune-inflate | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 1 | 156.5 KiB, 16587 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| enwik8 | 6 | 156.5 KiB, 15483 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| enwik8 | 9 | 156.5 KiB, 15447 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| enwik8 | 12 | 156.5 KiB, 47852 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| linux-6.12.tar | 1 | 156.5 KiB, 9603 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| linux-6.12.tar | 6 | 156.5 KiB, 8318 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| linux-6.12.tar | 9 | 156.5 KiB, 8258 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| linux-6.12.tar | 12 | 156.5 KiB, 32770 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |

#### Canterbury

| Input | Level | ours | zlib | zlib-ng | libdeflate | zlib-rs | miniz_oxide | zune-inflate | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 1 | 156.4 KiB, 38 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 297.1 KiB, 2 | 0.0 KiB, 0 |
| alice29.txt | 6 | 156.4 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 297.1 KiB, 2 | 0.0 KiB, 0 |
| alice29.txt | 9 | 156.4 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 297.1 KiB, 2 | 0.0 KiB, 0 |
| alice29.txt | 12 | 139.1 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 297.1 KiB, 2 | 0.0 KiB, 0 |
| asyoulik.txt | 1 | 139.1 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 244.5 KiB, 2 | 0.0 KiB, 0 |
| asyoulik.txt | 6 | 156.4 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 244.5 KiB, 2 | 0.0 KiB, 0 |
| asyoulik.txt | 9 | 156.5 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 244.5 KiB, 2 | 0.0 KiB, 0 |
| asyoulik.txt | 12 | 139.1 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 244.5 KiB, 2 | 0.0 KiB, 0 |
| cp.html | 1 | 49.6 KiB, 13 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 48.1 KiB, 2 | 0.0 KiB, 0 |
| cp.html | 6 | 49.6 KiB, 13 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 48.1 KiB, 2 | 0.0 KiB, 0 |
| cp.html | 9 | 49.6 KiB, 13 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 48.1 KiB, 2 | 0.0 KiB, 0 |
| cp.html | 12 | 59.6 KiB, 25 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 48.1 KiB, 2 | 0.0 KiB, 0 |
| fields.c | 1 | 49.6 KiB, 13 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 21.8 KiB, 2 | 0.0 KiB, 0 |
| fields.c | 6 | 49.6 KiB, 12 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 21.8 KiB, 2 | 0.0 KiB, 0 |
| fields.c | 9 | 49.6 KiB, 12 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 21.8 KiB, 2 | 0.0 KiB, 0 |
| fields.c | 12 | 49.7 KiB, 13 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 21.8 KiB, 2 | 0.0 KiB, 0 |
| grammar.lsp | 1 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7.9 KiB, 2 | 0.0 KiB, 0 |
| grammar.lsp | 6 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7.9 KiB, 2 | 0.0 KiB, 0 |
| grammar.lsp | 9 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7.9 KiB, 2 | 0.0 KiB, 0 |
| grammar.lsp | 12 | 45.6 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7.9 KiB, 2 | 0.0 KiB, 0 |
| kennedy.xls | 1 | 156.2 KiB, 134 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 2011.2 KiB, 2 | 0.0 KiB, 0 |
| kennedy.xls | 6 | 156.2 KiB, 123 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 2011.2 KiB, 2 | 0.0 KiB, 0 |
| kennedy.xls | 9 | 156.2 KiB, 123 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 2011.2 KiB, 2 | 0.0 KiB, 0 |
| kennedy.xls | 12 | 156.2 KiB, 814 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 2011.2 KiB, 2 | 0.0 KiB, 0 |
| lcet10.txt | 1 | 156.5 KiB, 75 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 833.5 KiB, 2 | 0.0 KiB, 0 |
| lcet10.txt | 6 | 156.5 KiB, 63 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 833.5 KiB, 2 | 0.0 KiB, 0 |
| lcet10.txt | 9 | 156.5 KiB, 63 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 833.5 KiB, 2 | 0.0 KiB, 0 |
| lcet10.txt | 12 | 156.5 KiB, 75 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 833.5 KiB, 2 | 0.0 KiB, 0 |
| plrabn12.txt | 1 | 156.4 KiB, 99 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 941.1 KiB, 2 | 0.0 KiB, 0 |
| plrabn12.txt | 6 | 156.5 KiB, 87 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 941.1 KiB, 2 | 0.0 KiB, 0 |
| plrabn12.txt | 9 | 156.5 KiB, 87 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 941.1 KiB, 2 | 0.0 KiB, 0 |
| plrabn12.txt | 12 | 156.4 KiB, 51 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 941.1 KiB, 2 | 0.0 KiB, 0 |
| ptt5 | 1 | 156.5 KiB, 39 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1002.4 KiB, 2 | 0.0 KiB, 0 |
| ptt5 | 6 | 156.5 KiB, 39 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1002.4 KiB, 2 | 0.0 KiB, 0 |
| ptt5 | 9 | 156.5 KiB, 39 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1002.4 KiB, 2 | 0.0 KiB, 0 |
| ptt5 | 12 | 156.5 KiB, 202 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1002.4 KiB, 2 | 0.0 KiB, 0 |
| sum | 1 | 73.6 KiB, 13 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 74.7 KiB, 2 | 0.0 KiB, 0 |
| sum | 6 | 74.6 KiB, 14 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 74.7 KiB, 2 | 0.0 KiB, 0 |
| sum | 9 | 74.6 KiB, 14 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 74.7 KiB, 2 | 0.0 KiB, 0 |
| sum | 12 | 74.6 KiB, 38 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 74.7 KiB, 2 | 0.0 KiB, 0 |
| xargs.1 | 1 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 8.4 KiB, 2 | 0.0 KiB, 0 |
| xargs.1 | 6 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 8.4 KiB, 2 | 0.0 KiB, 0 |
| xargs.1 | 9 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 8.4 KiB, 2 | 0.0 KiB, 0 |
| xargs.1 | 12 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 8.4 KiB, 2 | 0.0 KiB, 0 |
| E.coli | 1 | 156.4 KiB, 799 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 9059.9 KiB, 2 | 0.0 KiB, 0 |
| E.coli | 6 | 156.5 KiB, 603 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 9059.9 KiB, 2 | 0.0 KiB, 0 |
| E.coli | 9 | 156.5 KiB, 536 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 9059.9 KiB, 2 | 0.0 KiB, 0 |
| E.coli | 12 | 156.5 KiB, 207 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 9059.9 KiB, 2 | 0.0 KiB, 0 |
| bible.txt | 1 | 156.5 KiB, 555 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7905.1 KiB, 2 | 0.0 KiB, 0 |
| bible.txt | 6 | 156.5 KiB, 459 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7905.1 KiB, 2 | 0.0 KiB, 0 |
| bible.txt | 9 | 156.5 KiB, 447 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7905.1 KiB, 2 | 0.0 KiB, 0 |
| bible.txt | 12 | 156.5 KiB, 288 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7905.1 KiB, 2 | 0.0 KiB, 0 |
| world192.txt | 1 | 156.5 KiB, 363 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 4830.9 KiB, 2 | 0.0 KiB, 0 |
| world192.txt | 6 | 156.5 KiB, 315 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 4830.9 KiB, 2 | 0.0 KiB, 0 |
| world192.txt | 9 | 156.5 KiB, 315 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 4830.9 KiB, 2 | 0.0 KiB, 0 |
| world192.txt | 12 | 156.5 KiB, 602 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 4830.9 KiB, 2 | 0.0 KiB, 0 |

#### Rust's executables

| Input | Level | ours | zlib | zlib-ng | libdeflate | zlib-rs | miniz_oxide | zune-inflate | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| rustc | 1 | 156.5 KiB, 144 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1262.4 KiB, 2 | 0.0 KiB, 0 |
| rustc | 6 | 156.5 KiB, 135 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1262.4 KiB, 2 | 0.0 KiB, 0 |
| rustc | 9 | 156.5 KiB, 135 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1262.4 KiB, 2 | 0.0 KiB, 0 |
| rustc | 12 | 156.5 KiB, 209 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1262.4 KiB, 2 | 0.0 KiB, 0 |
| librustc_driver.so | 1 | 156.5 KiB, 32274 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 308356.0 KiB, 2 | 0.0 KiB, 0 |
| librustc_driver.so | 6 | 156.5 KiB, 30960 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 308356.0 KiB, 2 | 0.0 KiB, 0 |
| librustc_driver.so | 9 | 156.5 KiB, 30728 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 308356.0 KiB, 2 | 0.0 KiB, 0 |
| librustc_driver.so | 12 | 156.5 KiB, 32360 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 308356.0 KiB, 2 | 0.0 KiB, 0 |
| cargo | 1 | 156.5 KiB, 8327 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 83684.0 KiB, 2 | 0.0 KiB, 0 |
| cargo | 6 | 156.5 KiB, 7884 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 83684.0 KiB, 2 | 0.0 KiB, 0 |
| cargo | 9 | 156.5 KiB, 7811 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 83684.0 KiB, 2 | 0.0 KiB, 0 |
| cargo | 12 | 156.5 KiB, 11109 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 83684.0 KiB, 2 | 0.0 KiB, 0 |

Ours relative to zlib's peak: 22.38x at the median, and at most 22.38x.

### AMD EPYC 9R45 (1 vCPU under EC2 Nitro), in pieces of 4 KiB

Measured 2026-11-15, at `9e96979`, `decode-deflate` 0.0.4.

The size of each decoder's value: ours 168 B, zlib 112 B, zlib-ng 104 B,
zlib-rs 128 B, miniz_oxide 8 B, yazi 7296 B.

#### Synthetic

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| random | 1 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| random | 6 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| random | 9 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| random | 12 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 1 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 6 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 9 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 12 | 147.5 KiB, 283 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 1 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 6 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 9 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 12 | 147.5 KiB, 283 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 1 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 6 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 9 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 12 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Silesia

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| dickens | 1 | 156.5 KiB, 1731 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| dickens | 6 | 156.5 KiB, 1527 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| dickens | 9 | 156.5 KiB, 1515 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| dickens | 12 | 156.5 KiB, 639 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 1 | 156.5 KiB, 10485 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 6 | 156.5 KiB, 9989 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 9 | 156.5 KiB, 9960 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 12 | 156.5 KiB, 23342 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 1 | 156.5 KiB, 1576 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 6 | 156.5 KiB, 1662 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 9 | 156.5 KiB, 1670 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 12 | 156.5 KiB, 1362 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 1 | 156.5 KiB, 1671 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 6 | 156.5 KiB, 1311 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 9 | 156.5 KiB, 1311 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 12 | 156.5 KiB, 3853 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 1 | 156.5 KiB, 1635 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 6 | 156.5 KiB, 1605 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 9 | 156.5 KiB, 1597 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 12 | 156.5 KiB, 1260 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 1 | 156.4 KiB, 2115 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 6 | 156.4 KiB, 1987 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 9 | 156.4 KiB, 1982 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 12 | 156.4 KiB, 423 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 1 | 156.5 KiB, 915 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 6 | 156.5 KiB, 702 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 9 | 156.5 KiB, 699 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 12 | 155.5 KiB, 344 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 1 | 156.5 KiB, 2859 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 6 | 156.5 KiB, 2629 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 9 | 156.5 KiB, 2612 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 12 | 156.5 KiB, 8936 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 1 | 156.2 KiB, 3016 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 6 | 156.2 KiB, 3030 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 9 | 156.2 KiB, 3041 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 12 | 156.2 KiB, 303 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 1 | 156.5 KiB, 5559 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 6 | 156.5 KiB, 4983 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 9 | 156.5 KiB, 4959 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 12 | 156.5 KiB, 1933 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 1 | 156.5 KiB, 375 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 6 | 156.5 KiB, 291 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 9 | 156.5 KiB, 291 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 12 | 156.5 KiB, 705 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 1 | 156.2 KiB, 2677 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 6 | 156.2 KiB, 3081 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 9 | 156.2 KiB, 3081 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 12 | 156.2 KiB, 409 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Large files

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 1 | 156.5 KiB, 16587 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| enwik8 | 6 | 156.5 KiB, 15483 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| enwik8 | 9 | 156.5 KiB, 15447 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| enwik8 | 12 | 156.5 KiB, 47852 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 1 | 156.5 KiB, 9603 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 6 | 156.5 KiB, 8318 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 9 | 156.5 KiB, 8258 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 12 | 156.5 KiB, 32770 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Canterbury

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 1 | 156.4 KiB, 38 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| alice29.txt | 6 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| alice29.txt | 9 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| alice29.txt | 12 | 139.1 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 1 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 6 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 9 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 12 | 139.1 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 1 | 74.6 KiB, 14 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 6 | 74.6 KiB, 14 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 9 | 74.6 KiB, 14 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 12 | 91.9 KiB, 26 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 1 | 49.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 6 | 49.6 KiB, 12 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 9 | 49.6 KiB, 12 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 12 | 49.7 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 1 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 6 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 9 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 12 | 45.6 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 1 | 156.2 KiB, 134 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 6 | 156.2 KiB, 123 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 9 | 156.2 KiB, 123 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 12 | 156.2 KiB, 814 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 1 | 156.5 KiB, 75 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 6 | 156.5 KiB, 63 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 9 | 156.5 KiB, 63 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 12 | 156.5 KiB, 75 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 1 | 156.4 KiB, 99 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 6 | 156.5 KiB, 87 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 9 | 156.5 KiB, 87 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 12 | 156.4 KiB, 51 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 1 | 156.5 KiB, 39 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 6 | 156.5 KiB, 39 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 9 | 156.5 KiB, 39 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 12 | 156.5 KiB, 202 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 1 | 73.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 6 | 139.1 KiB, 15 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 9 | 139.1 KiB, 15 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 12 | 92.0 KiB, 38 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 1 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 6 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 9 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 12 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 1 | 156.5 KiB, 799 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 6 | 156.5 KiB, 603 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 9 | 156.5 KiB, 536 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 12 | 156.5 KiB, 207 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 1 | 156.5 KiB, 555 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 6 | 156.5 KiB, 459 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 9 | 156.5 KiB, 447 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 12 | 156.5 KiB, 288 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 1 | 156.5 KiB, 363 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 6 | 156.5 KiB, 315 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 9 | 156.5 KiB, 315 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 12 | 156.5 KiB, 602 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Rust's executables

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| rustc | 1 | 156.5 KiB, 144 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| rustc | 6 | 156.5 KiB, 135 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| rustc | 9 | 156.5 KiB, 135 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| rustc | 12 | 156.5 KiB, 209 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 1 | 156.5 KiB, 32274 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 6 | 156.5 KiB, 30960 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 9 | 156.5 KiB, 30728 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 12 | 156.5 KiB, 32360 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 1 | 156.5 KiB, 8327 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 6 | 156.5 KiB, 7884 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 9 | 156.5 KiB, 7811 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 12 | 156.5 KiB, 11109 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

Ours relative to zlib's peak: 4.01x at the median, and at most 4.01x.

### AMD EPYC 9R45 (1 vCPU under EC2 Nitro), in pieces of 64 KiB

Measured 2026-11-15, at `9e96979`, `decode-deflate` 0.0.4.

The size of each decoder's value: ours 168 B, zlib 112 B, zlib-ng 104 B,
zlib-rs 128 B, miniz_oxide 8 B, yazi 7296 B.

#### Synthetic

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| random | 1 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| random | 6 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| random | 9 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| random | 12 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 1 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 6 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 9 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 12 | 147.5 KiB, 283 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 1 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 6 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 9 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 12 | 147.5 KiB, 283 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 1 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 6 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 9 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 12 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Silesia

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| dickens | 1 | 156.5 KiB, 1731 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| dickens | 6 | 156.5 KiB, 1527 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| dickens | 9 | 156.5 KiB, 1515 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| dickens | 12 | 156.5 KiB, 639 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 1 | 156.5 KiB, 10485 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 6 | 156.5 KiB, 9989 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 9 | 156.5 KiB, 9960 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 12 | 156.5 KiB, 23342 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 1 | 156.5 KiB, 1576 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 6 | 156.5 KiB, 1662 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 9 | 156.5 KiB, 1670 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 12 | 156.5 KiB, 1362 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 1 | 156.5 KiB, 1671 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 6 | 156.5 KiB, 1311 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 9 | 156.5 KiB, 1311 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 12 | 156.5 KiB, 3853 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 1 | 156.5 KiB, 1635 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 6 | 156.5 KiB, 1605 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 9 | 156.5 KiB, 1597 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 12 | 156.5 KiB, 1260 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 1 | 156.4 KiB, 2115 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 6 | 156.4 KiB, 1987 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 9 | 156.4 KiB, 1982 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 12 | 156.4 KiB, 423 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 1 | 156.5 KiB, 915 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 6 | 156.5 KiB, 702 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 9 | 156.5 KiB, 699 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 12 | 155.5 KiB, 344 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 1 | 156.5 KiB, 2859 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 6 | 156.5 KiB, 2629 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 9 | 156.5 KiB, 2612 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 12 | 156.5 KiB, 8936 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 1 | 156.2 KiB, 3016 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 6 | 156.2 KiB, 3030 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 9 | 156.2 KiB, 3041 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 12 | 156.2 KiB, 303 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 1 | 156.5 KiB, 5559 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 6 | 156.5 KiB, 4983 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 9 | 156.5 KiB, 4959 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 12 | 156.5 KiB, 1933 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 1 | 156.5 KiB, 375 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 6 | 156.5 KiB, 291 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 9 | 156.5 KiB, 291 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 12 | 156.5 KiB, 705 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 1 | 156.2 KiB, 2677 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 6 | 156.2 KiB, 3081 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 9 | 156.2 KiB, 3081 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 12 | 156.2 KiB, 409 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Large files

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 1 | 156.5 KiB, 16587 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| enwik8 | 6 | 156.5 KiB, 15483 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| enwik8 | 9 | 156.5 KiB, 15447 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| enwik8 | 12 | 156.5 KiB, 47852 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 1 | 156.5 KiB, 9603 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 6 | 156.5 KiB, 8318 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 9 | 156.5 KiB, 8258 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 12 | 156.5 KiB, 32770 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Canterbury

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 1 | 156.4 KiB, 38 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| alice29.txt | 6 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| alice29.txt | 9 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| alice29.txt | 12 | 139.1 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 1 | 139.1 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 6 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 9 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 12 | 139.1 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 1 | 49.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 6 | 49.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 9 | 49.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 12 | 59.6 KiB, 25 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 1 | 49.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 6 | 49.6 KiB, 12 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 9 | 49.6 KiB, 12 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 12 | 49.7 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 1 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 6 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 9 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 12 | 45.6 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 1 | 156.2 KiB, 134 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 6 | 156.2 KiB, 123 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 9 | 156.2 KiB, 123 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 12 | 156.2 KiB, 814 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 1 | 156.5 KiB, 75 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 6 | 156.5 KiB, 63 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 9 | 156.5 KiB, 63 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 12 | 156.5 KiB, 75 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 1 | 156.4 KiB, 99 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 6 | 156.5 KiB, 87 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 9 | 156.5 KiB, 87 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 12 | 156.4 KiB, 51 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 1 | 156.5 KiB, 39 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 6 | 156.5 KiB, 39 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 9 | 156.5 KiB, 39 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 12 | 156.5 KiB, 202 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 1 | 73.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 6 | 74.6 KiB, 14 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 9 | 74.6 KiB, 14 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 12 | 74.6 KiB, 38 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 1 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 6 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 9 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 12 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 1 | 156.5 KiB, 799 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 6 | 156.5 KiB, 603 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 9 | 156.5 KiB, 536 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 12 | 156.5 KiB, 207 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 1 | 156.5 KiB, 555 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 6 | 156.5 KiB, 459 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 9 | 156.5 KiB, 447 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 12 | 156.5 KiB, 288 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 1 | 156.5 KiB, 363 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 6 | 156.5 KiB, 315 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 9 | 156.5 KiB, 315 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 12 | 156.5 KiB, 602 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Rust's executables

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| rustc | 1 | 156.5 KiB, 144 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| rustc | 6 | 156.5 KiB, 135 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| rustc | 9 | 156.5 KiB, 135 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| rustc | 12 | 156.5 KiB, 209 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 1 | 156.5 KiB, 32274 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 6 | 156.5 KiB, 30960 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 9 | 156.5 KiB, 30728 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 12 | 156.5 KiB, 32360 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 1 | 156.5 KiB, 8327 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 6 | 156.5 KiB, 7884 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 9 | 156.5 KiB, 7811 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 12 | 156.5 KiB, 11109 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

Ours relative to zlib's peak: 4.01x at the median, and at most 4.01x.

### ARM Neoverse-V3 (1 vCPU under EC2 Nitro), whole

Measured 2026-11-15, at `9e96979`, `decode-deflate` 0.0.4.

The size of each decoder's value: ours 168 B, zlib 112 B, zlib-ng 104 B,
libdeflate 8 B, zlib-rs 128 B, miniz_oxide 8 B, zune-inflate 11520 B,
yazi 7296 B.

#### Synthetic

| Input | Level | ours | zlib | zlib-ng | libdeflate | zlib-rs | miniz_oxide | zune-inflate | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| random | 1 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16388.0 KiB, 2 | 0.0 KiB, 0 |
| random | 6 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| random | 9 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| random | 12 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| zeros | 1 | 147.5 KiB, 23 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| zeros | 6 | 147.5 KiB, 23 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| zeros | 9 | 147.5 KiB, 23 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| zeros | 12 | 147.5 KiB, 283 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| pattern | 1 | 147.5 KiB, 23 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| pattern | 6 | 147.5 KiB, 23 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| pattern | 9 | 147.5 KiB, 23 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| pattern | 12 | 147.5 KiB, 283 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| far | 1 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| far | 6 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| far | 9 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| far | 12 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |

#### Silesia

| Input | Level | ours | zlib | zlib-ng | libdeflate | zlib-rs | miniz_oxide | zune-inflate | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| dickens | 1 | 156.5 KiB, 1731 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19908.0 KiB, 2 | 0.0 KiB, 0 |
| dickens | 6 | 156.5 KiB, 1527 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19907.1 KiB, 2 | 0.0 KiB, 0 |
| dickens | 9 | 156.5 KiB, 1515 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19907.1 KiB, 2 | 0.0 KiB, 0 |
| dickens | 12 | 156.5 KiB, 639 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19907.1 KiB, 2 | 0.0 KiB, 0 |
| mozilla | 1 | 156.5 KiB, 10485 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 100044.0 KiB, 2 | 0.0 KiB, 0 |
| mozilla | 6 | 156.5 KiB, 9989 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 100044.0 KiB, 2 | 0.0 KiB, 0 |
| mozilla | 9 | 156.5 KiB, 9960 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 100044.0 KiB, 2 | 0.0 KiB, 0 |
| mozilla | 12 | 156.5 KiB, 23342 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 100044.0 KiB, 2 | 0.0 KiB, 0 |
| mr | 1 | 156.5 KiB, 1576 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19473.8 KiB, 2 | 0.0 KiB, 0 |
| mr | 6 | 156.5 KiB, 1662 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19473.8 KiB, 2 | 0.0 KiB, 0 |
| mr | 9 | 156.5 KiB, 1670 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19473.8 KiB, 2 | 0.0 KiB, 0 |
| mr | 12 | 156.5 KiB, 1362 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19473.8 KiB, 2 | 0.0 KiB, 0 |
| nci | 1 | 156.5 KiB, 1671 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 65536.0 KiB, 2 | 0.0 KiB, 0 |
| nci | 6 | 156.5 KiB, 1311 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 65536.0 KiB, 2 | 0.0 KiB, 0 |
| nci | 9 | 156.5 KiB, 1311 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 65536.0 KiB, 2 | 0.0 KiB, 0 |
| nci | 12 | 156.5 KiB, 3853 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 65536.0 KiB, 2 | 0.0 KiB, 0 |
| ooffice | 1 | 156.5 KiB, 1635 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12016.0 KiB, 2 | 0.0 KiB, 0 |
| ooffice | 6 | 156.5 KiB, 1605 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12016.0 KiB, 2 | 0.0 KiB, 0 |
| ooffice | 9 | 156.5 KiB, 1597 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12016.0 KiB, 2 | 0.0 KiB, 0 |
| ooffice | 12 | 156.5 KiB, 1260 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12016.0 KiB, 2 | 0.0 KiB, 0 |
| osdb | 1 | 156.4 KiB, 2115 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19698.6 KiB, 2 | 0.0 KiB, 0 |
| osdb | 6 | 156.4 KiB, 1987 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19698.6 KiB, 2 | 0.0 KiB, 0 |
| osdb | 9 | 156.4 KiB, 1982 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19698.6 KiB, 2 | 0.0 KiB, 0 |
| osdb | 12 | 156.4 KiB, 423 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19698.6 KiB, 2 | 0.0 KiB, 0 |
| reymont | 1 | 156.5 KiB, 915 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12943.8 KiB, 2 | 0.0 KiB, 0 |
| reymont | 6 | 156.5 KiB, 702 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12943.8 KiB, 2 | 0.0 KiB, 0 |
| reymont | 9 | 156.5 KiB, 699 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12943.8 KiB, 2 | 0.0 KiB, 0 |
| reymont | 12 | 155.5 KiB, 344 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12943.8 KiB, 2 | 0.0 KiB, 0 |
| samba | 1 | 156.5 KiB, 2859 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 42204.0 KiB, 2 | 0.0 KiB, 0 |
| samba | 6 | 156.5 KiB, 2629 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 42204.0 KiB, 2 | 0.0 KiB, 0 |
| samba | 9 | 156.5 KiB, 2612 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 42204.0 KiB, 2 | 0.0 KiB, 0 |
| samba | 12 | 156.5 KiB, 8936 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 42204.0 KiB, 2 | 0.0 KiB, 0 |
| sao | 1 | 156.2 KiB, 3016 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 14164.0 KiB, 2 | 0.0 KiB, 0 |
| sao | 6 | 156.2 KiB, 3030 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 14164.0 KiB, 2 | 0.0 KiB, 0 |
| sao | 9 | 156.2 KiB, 3041 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 14164.0 KiB, 2 | 0.0 KiB, 0 |
| sao | 12 | 156.2 KiB, 303 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 14164.0 KiB, 2 | 0.0 KiB, 0 |
| webster | 1 | 156.5 KiB, 5559 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 80976.0 KiB, 2 | 0.0 KiB, 0 |
| webster | 6 | 156.5 KiB, 4983 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 80976.0 KiB, 2 | 0.0 KiB, 0 |
| webster | 9 | 156.5 KiB, 4959 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 80976.0 KiB, 2 | 0.0 KiB, 0 |
| webster | 12 | 156.5 KiB, 1933 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 80976.0 KiB, 2 | 0.0 KiB, 0 |
| xml | 1 | 156.5 KiB, 375 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 10440.0 KiB, 2 | 0.0 KiB, 0 |
| xml | 6 | 156.5 KiB, 291 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 10440.0 KiB, 2 | 0.0 KiB, 0 |
| xml | 9 | 156.5 KiB, 291 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 10440.0 KiB, 2 | 0.0 KiB, 0 |
| xml | 12 | 156.5 KiB, 705 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 10440.0 KiB, 2 | 0.0 KiB, 0 |
| x-ray | 1 | 156.2 KiB, 2677 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16551.3 KiB, 2 | 0.0 KiB, 0 |
| x-ray | 6 | 156.2 KiB, 3081 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16551.3 KiB, 2 | 0.0 KiB, 0 |
| x-ray | 9 | 156.2 KiB, 3081 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16551.3 KiB, 2 | 0.0 KiB, 0 |
| x-ray | 12 | 156.2 KiB, 409 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16551.3 KiB, 2 | 0.0 KiB, 0 |

#### Large files

| Input | Level | ours | zlib | zlib-ng | libdeflate | zlib-rs | miniz_oxide | zune-inflate | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 1 | 156.5 KiB, 16587 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| enwik8 | 6 | 156.5 KiB, 15483 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| enwik8 | 9 | 156.5 KiB, 15447 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| enwik8 | 12 | 156.5 KiB, 47852 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| linux-6.12.tar | 1 | 156.5 KiB, 9603 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| linux-6.12.tar | 6 | 156.5 KiB, 8318 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| linux-6.12.tar | 9 | 156.5 KiB, 8258 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| linux-6.12.tar | 12 | 156.5 KiB, 32770 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |

#### Canterbury

| Input | Level | ours | zlib | zlib-ng | libdeflate | zlib-rs | miniz_oxide | zune-inflate | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 1 | 156.4 KiB, 38 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 297.1 KiB, 2 | 0.0 KiB, 0 |
| alice29.txt | 6 | 156.4 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 297.1 KiB, 2 | 0.0 KiB, 0 |
| alice29.txt | 9 | 156.4 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 297.1 KiB, 2 | 0.0 KiB, 0 |
| alice29.txt | 12 | 139.1 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 297.1 KiB, 2 | 0.0 KiB, 0 |
| asyoulik.txt | 1 | 139.1 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 244.5 KiB, 2 | 0.0 KiB, 0 |
| asyoulik.txt | 6 | 156.4 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 244.5 KiB, 2 | 0.0 KiB, 0 |
| asyoulik.txt | 9 | 156.4 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 244.5 KiB, 2 | 0.0 KiB, 0 |
| asyoulik.txt | 12 | 139.1 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 244.5 KiB, 2 | 0.0 KiB, 0 |
| cp.html | 1 | 49.6 KiB, 13 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 48.1 KiB, 2 | 0.0 KiB, 0 |
| cp.html | 6 | 49.6 KiB, 13 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 48.1 KiB, 2 | 0.0 KiB, 0 |
| cp.html | 9 | 49.6 KiB, 13 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 48.1 KiB, 2 | 0.0 KiB, 0 |
| cp.html | 12 | 59.6 KiB, 25 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 48.1 KiB, 2 | 0.0 KiB, 0 |
| fields.c | 1 | 49.6 KiB, 13 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 21.8 KiB, 2 | 0.0 KiB, 0 |
| fields.c | 6 | 49.6 KiB, 12 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 21.8 KiB, 2 | 0.0 KiB, 0 |
| fields.c | 9 | 49.6 KiB, 12 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 21.8 KiB, 2 | 0.0 KiB, 0 |
| fields.c | 12 | 49.7 KiB, 13 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 21.8 KiB, 2 | 0.0 KiB, 0 |
| grammar.lsp | 1 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7.9 KiB, 2 | 0.0 KiB, 0 |
| grammar.lsp | 6 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7.9 KiB, 2 | 0.0 KiB, 0 |
| grammar.lsp | 9 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7.9 KiB, 2 | 0.0 KiB, 0 |
| grammar.lsp | 12 | 45.6 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7.9 KiB, 2 | 0.0 KiB, 0 |
| kennedy.xls | 1 | 156.2 KiB, 134 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 2011.2 KiB, 2 | 0.0 KiB, 0 |
| kennedy.xls | 6 | 156.2 KiB, 123 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 2011.2 KiB, 2 | 0.0 KiB, 0 |
| kennedy.xls | 9 | 156.2 KiB, 123 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 2011.2 KiB, 2 | 0.0 KiB, 0 |
| kennedy.xls | 12 | 156.2 KiB, 814 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 2011.2 KiB, 2 | 0.0 KiB, 0 |
| lcet10.txt | 1 | 156.5 KiB, 75 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 833.5 KiB, 2 | 0.0 KiB, 0 |
| lcet10.txt | 6 | 156.5 KiB, 63 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 833.5 KiB, 2 | 0.0 KiB, 0 |
| lcet10.txt | 9 | 156.5 KiB, 63 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 833.5 KiB, 2 | 0.0 KiB, 0 |
| lcet10.txt | 12 | 156.5 KiB, 75 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 833.5 KiB, 2 | 0.0 KiB, 0 |
| plrabn12.txt | 1 | 156.4 KiB, 99 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 941.1 KiB, 2 | 0.0 KiB, 0 |
| plrabn12.txt | 6 | 156.5 KiB, 87 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 941.1 KiB, 2 | 0.0 KiB, 0 |
| plrabn12.txt | 9 | 156.5 KiB, 87 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 941.1 KiB, 2 | 0.0 KiB, 0 |
| plrabn12.txt | 12 | 156.4 KiB, 51 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 941.1 KiB, 2 | 0.0 KiB, 0 |
| ptt5 | 1 | 156.5 KiB, 39 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1002.4 KiB, 2 | 0.0 KiB, 0 |
| ptt5 | 6 | 156.5 KiB, 39 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1002.4 KiB, 2 | 0.0 KiB, 0 |
| ptt5 | 9 | 156.5 KiB, 39 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1002.4 KiB, 2 | 0.0 KiB, 0 |
| ptt5 | 12 | 156.5 KiB, 202 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1002.4 KiB, 2 | 0.0 KiB, 0 |
| sum | 1 | 73.6 KiB, 13 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 74.7 KiB, 2 | 0.0 KiB, 0 |
| sum | 6 | 74.6 KiB, 14 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 74.7 KiB, 2 | 0.0 KiB, 0 |
| sum | 9 | 74.6 KiB, 14 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 74.7 KiB, 2 | 0.0 KiB, 0 |
| sum | 12 | 74.6 KiB, 38 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 74.7 KiB, 2 | 0.0 KiB, 0 |
| xargs.1 | 1 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 8.4 KiB, 2 | 0.0 KiB, 0 |
| xargs.1 | 6 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 8.4 KiB, 2 | 0.0 KiB, 0 |
| xargs.1 | 9 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 8.4 KiB, 2 | 0.0 KiB, 0 |
| xargs.1 | 12 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 8.4 KiB, 2 | 0.0 KiB, 0 |
| E.coli | 1 | 156.4 KiB, 799 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 9059.9 KiB, 2 | 0.0 KiB, 0 |
| E.coli | 6 | 156.5 KiB, 603 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 9059.9 KiB, 2 | 0.0 KiB, 0 |
| E.coli | 9 | 156.5 KiB, 536 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 9059.9 KiB, 2 | 0.0 KiB, 0 |
| E.coli | 12 | 156.5 KiB, 207 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 9059.9 KiB, 2 | 0.0 KiB, 0 |
| bible.txt | 1 | 156.5 KiB, 555 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7905.1 KiB, 2 | 0.0 KiB, 0 |
| bible.txt | 6 | 156.5 KiB, 459 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7905.1 KiB, 2 | 0.0 KiB, 0 |
| bible.txt | 9 | 156.5 KiB, 447 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7905.1 KiB, 2 | 0.0 KiB, 0 |
| bible.txt | 12 | 156.5 KiB, 288 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7905.1 KiB, 2 | 0.0 KiB, 0 |
| world192.txt | 1 | 156.5 KiB, 363 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 4830.9 KiB, 2 | 0.0 KiB, 0 |
| world192.txt | 6 | 156.5 KiB, 315 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 4830.9 KiB, 2 | 0.0 KiB, 0 |
| world192.txt | 9 | 156.5 KiB, 315 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 4830.9 KiB, 2 | 0.0 KiB, 0 |
| world192.txt | 12 | 156.5 KiB, 602 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 4830.9 KiB, 2 | 0.0 KiB, 0 |

#### Rust's executables

| Input | Level | ours | zlib | zlib-ng | libdeflate | zlib-rs | miniz_oxide | zune-inflate | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| rustc | 1 | 156.5 KiB, 144 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1262.4 KiB, 2 | 0.0 KiB, 0 |
| rustc | 6 | 156.5 KiB, 135 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1262.4 KiB, 2 | 0.0 KiB, 0 |
| rustc | 9 | 156.5 KiB, 135 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1262.4 KiB, 2 | 0.0 KiB, 0 |
| rustc | 12 | 156.5 KiB, 209 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1262.4 KiB, 2 | 0.0 KiB, 0 |
| librustc_driver.so | 1 | 156.5 KiB, 32274 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 308356.0 KiB, 2 | 0.0 KiB, 0 |
| librustc_driver.so | 6 | 156.5 KiB, 30960 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 308356.0 KiB, 2 | 0.0 KiB, 0 |
| librustc_driver.so | 9 | 156.5 KiB, 30728 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 308356.0 KiB, 2 | 0.0 KiB, 0 |
| librustc_driver.so | 12 | 156.5 KiB, 32360 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 308356.0 KiB, 2 | 0.0 KiB, 0 |
| cargo | 1 | 156.5 KiB, 8327 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 83684.0 KiB, 2 | 0.0 KiB, 0 |
| cargo | 6 | 156.5 KiB, 7884 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 83684.0 KiB, 2 | 0.0 KiB, 0 |
| cargo | 9 | 156.5 KiB, 7811 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 83684.0 KiB, 2 | 0.0 KiB, 0 |
| cargo | 12 | 156.5 KiB, 11109 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 83684.0 KiB, 2 | 0.0 KiB, 0 |

Ours relative to zlib's peak: 22.38x at the median, and at most 22.38x.

### ARM Neoverse-V3 (1 vCPU under EC2 Nitro), in pieces of 4 KiB

Measured 2026-11-15, at `9e96979`, `decode-deflate` 0.0.4.

The size of each decoder's value: ours 168 B, zlib 112 B, zlib-ng 104 B,
zlib-rs 128 B, miniz_oxide 8 B, yazi 7296 B.

#### Synthetic

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| random | 1 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| random | 6 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| random | 9 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| random | 12 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 1 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 6 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 9 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 12 | 147.5 KiB, 283 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 1 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 6 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 9 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 12 | 147.5 KiB, 283 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 1 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 6 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 9 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 12 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Silesia

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| dickens | 1 | 156.5 KiB, 1731 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| dickens | 6 | 156.5 KiB, 1527 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| dickens | 9 | 156.5 KiB, 1515 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| dickens | 12 | 156.5 KiB, 639 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 1 | 156.5 KiB, 10485 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 6 | 156.5 KiB, 9989 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 9 | 156.5 KiB, 9960 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 12 | 156.5 KiB, 23342 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 1 | 156.5 KiB, 1576 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 6 | 156.5 KiB, 1662 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 9 | 156.5 KiB, 1670 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 12 | 156.5 KiB, 1362 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 1 | 156.5 KiB, 1671 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 6 | 156.5 KiB, 1311 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 9 | 156.5 KiB, 1311 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 12 | 156.5 KiB, 3853 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 1 | 156.5 KiB, 1635 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 6 | 156.5 KiB, 1605 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 9 | 156.5 KiB, 1597 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 12 | 156.5 KiB, 1260 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 1 | 156.4 KiB, 2115 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 6 | 156.4 KiB, 1987 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 9 | 156.4 KiB, 1982 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 12 | 156.4 KiB, 423 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 1 | 156.5 KiB, 915 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 6 | 156.5 KiB, 702 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 9 | 156.5 KiB, 699 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 12 | 155.5 KiB, 344 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 1 | 156.5 KiB, 2859 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 6 | 156.5 KiB, 2629 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 9 | 156.5 KiB, 2612 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 12 | 156.5 KiB, 8936 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 1 | 156.2 KiB, 3016 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 6 | 156.2 KiB, 3030 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 9 | 156.2 KiB, 3041 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 12 | 156.2 KiB, 303 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 1 | 156.5 KiB, 5559 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 6 | 156.5 KiB, 4983 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 9 | 156.5 KiB, 4959 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 12 | 156.5 KiB, 1933 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 1 | 156.5 KiB, 375 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 6 | 156.5 KiB, 291 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 9 | 156.5 KiB, 291 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 12 | 156.5 KiB, 705 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 1 | 156.2 KiB, 2677 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 6 | 156.2 KiB, 3081 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 9 | 156.2 KiB, 3081 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 12 | 156.2 KiB, 409 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Large files

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 1 | 156.5 KiB, 16587 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| enwik8 | 6 | 156.5 KiB, 15483 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| enwik8 | 9 | 156.5 KiB, 15447 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| enwik8 | 12 | 156.5 KiB, 47852 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 1 | 156.5 KiB, 9603 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 6 | 156.5 KiB, 8318 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 9 | 156.5 KiB, 8258 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 12 | 156.5 KiB, 32770 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Canterbury

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 1 | 156.4 KiB, 38 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| alice29.txt | 6 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| alice29.txt | 9 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| alice29.txt | 12 | 139.1 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 1 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 6 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 9 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 12 | 139.1 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 1 | 74.6 KiB, 14 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 6 | 74.6 KiB, 14 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 9 | 74.6 KiB, 14 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 12 | 91.9 KiB, 26 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 1 | 49.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 6 | 49.6 KiB, 12 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 9 | 49.6 KiB, 12 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 12 | 49.7 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 1 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 6 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 9 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 12 | 45.6 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 1 | 156.2 KiB, 134 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 6 | 156.2 KiB, 123 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 9 | 156.2 KiB, 123 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 12 | 156.2 KiB, 814 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 1 | 156.5 KiB, 75 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 6 | 156.5 KiB, 63 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 9 | 156.5 KiB, 63 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 12 | 156.5 KiB, 75 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 1 | 156.4 KiB, 99 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 6 | 156.5 KiB, 87 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 9 | 156.5 KiB, 87 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 12 | 156.4 KiB, 51 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 1 | 156.5 KiB, 39 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 6 | 156.5 KiB, 39 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 9 | 156.5 KiB, 39 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 12 | 156.5 KiB, 202 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 1 | 73.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 6 | 139.1 KiB, 15 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 9 | 139.1 KiB, 15 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 12 | 92.0 KiB, 38 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 1 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 6 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 9 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 12 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 1 | 156.4 KiB, 799 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 6 | 156.5 KiB, 603 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 9 | 156.5 KiB, 536 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 12 | 156.5 KiB, 207 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 1 | 156.5 KiB, 555 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 6 | 156.5 KiB, 459 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 9 | 156.5 KiB, 447 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 12 | 156.5 KiB, 288 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 1 | 156.5 KiB, 363 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 6 | 156.5 KiB, 315 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 9 | 156.5 KiB, 315 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 12 | 156.5 KiB, 602 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Rust's executables

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| rustc | 1 | 156.5 KiB, 144 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| rustc | 6 | 156.5 KiB, 135 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| rustc | 9 | 156.5 KiB, 135 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| rustc | 12 | 156.5 KiB, 209 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 1 | 156.5 KiB, 32274 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 6 | 156.5 KiB, 30960 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 9 | 156.5 KiB, 30728 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 12 | 156.5 KiB, 32360 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 1 | 156.5 KiB, 8327 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 6 | 156.5 KiB, 7884 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 9 | 156.5 KiB, 7811 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 12 | 156.5 KiB, 11109 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

Ours relative to zlib's peak: 4.01x at the median, and at most 4.01x.

### ARM Neoverse-V3 (1 vCPU under EC2 Nitro), in pieces of 64 KiB

Measured 2026-11-15, at `9e96979`, `decode-deflate` 0.0.4.

The size of each decoder's value: ours 168 B, zlib 112 B, zlib-ng 104 B,
zlib-rs 128 B, miniz_oxide 8 B, yazi 7296 B.

#### Synthetic

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| random | 1 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| random | 6 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| random | 9 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| random | 12 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 1 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 6 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 9 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 12 | 147.5 KiB, 283 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 1 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 6 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 9 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 12 | 147.5 KiB, 283 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 1 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 6 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 9 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 12 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Silesia

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| dickens | 1 | 156.5 KiB, 1731 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| dickens | 6 | 156.5 KiB, 1527 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| dickens | 9 | 156.5 KiB, 1515 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| dickens | 12 | 156.5 KiB, 639 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 1 | 156.5 KiB, 10485 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 6 | 156.5 KiB, 9989 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 9 | 156.5 KiB, 9960 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 12 | 156.5 KiB, 23342 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 1 | 156.5 KiB, 1576 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 6 | 156.5 KiB, 1662 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 9 | 156.5 KiB, 1670 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 12 | 156.5 KiB, 1362 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 1 | 156.5 KiB, 1671 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 6 | 156.5 KiB, 1311 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 9 | 156.5 KiB, 1311 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 12 | 156.5 KiB, 3853 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 1 | 156.5 KiB, 1635 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 6 | 156.5 KiB, 1605 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 9 | 156.5 KiB, 1597 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 12 | 156.5 KiB, 1260 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 1 | 156.4 KiB, 2115 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 6 | 156.4 KiB, 1987 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 9 | 156.4 KiB, 1982 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 12 | 156.4 KiB, 423 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 1 | 156.5 KiB, 915 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 6 | 156.5 KiB, 702 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 9 | 156.5 KiB, 699 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 12 | 155.5 KiB, 344 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 1 | 156.5 KiB, 2859 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 6 | 156.5 KiB, 2629 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 9 | 156.5 KiB, 2612 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 12 | 156.5 KiB, 8936 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 1 | 156.2 KiB, 3016 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 6 | 156.2 KiB, 3030 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 9 | 156.2 KiB, 3041 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 12 | 156.2 KiB, 303 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 1 | 156.5 KiB, 5559 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 6 | 156.5 KiB, 4983 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 9 | 156.5 KiB, 4959 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 12 | 156.5 KiB, 1933 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 1 | 156.5 KiB, 375 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 6 | 156.5 KiB, 291 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 9 | 156.5 KiB, 291 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 12 | 156.5 KiB, 705 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 1 | 156.2 KiB, 2677 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 6 | 156.2 KiB, 3081 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 9 | 156.2 KiB, 3081 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 12 | 156.2 KiB, 409 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Large files

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 1 | 156.5 KiB, 16587 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| enwik8 | 6 | 156.5 KiB, 15483 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| enwik8 | 9 | 156.5 KiB, 15447 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| enwik8 | 12 | 156.5 KiB, 47852 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 1 | 156.5 KiB, 9603 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 6 | 156.5 KiB, 8318 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 9 | 156.5 KiB, 8258 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 12 | 156.5 KiB, 32770 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Canterbury

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 1 | 156.4 KiB, 38 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| alice29.txt | 6 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| alice29.txt | 9 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| alice29.txt | 12 | 139.1 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 1 | 139.1 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 6 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 9 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 12 | 139.1 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 1 | 49.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 6 | 49.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 9 | 49.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 12 | 59.6 KiB, 25 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 1 | 49.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 6 | 49.6 KiB, 12 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 9 | 49.6 KiB, 12 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 12 | 49.7 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 1 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 6 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 9 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 12 | 45.6 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 1 | 156.2 KiB, 134 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 6 | 156.2 KiB, 123 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 9 | 156.2 KiB, 123 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 12 | 156.2 KiB, 814 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 1 | 156.5 KiB, 75 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 6 | 156.5 KiB, 63 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 9 | 156.5 KiB, 63 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 12 | 156.5 KiB, 75 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 1 | 156.4 KiB, 99 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 6 | 156.5 KiB, 87 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 9 | 156.5 KiB, 87 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 12 | 156.4 KiB, 51 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 1 | 156.5 KiB, 39 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 6 | 156.5 KiB, 39 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 9 | 156.5 KiB, 39 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 12 | 156.5 KiB, 202 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 1 | 73.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 6 | 74.6 KiB, 14 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 9 | 74.6 KiB, 14 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 12 | 74.6 KiB, 38 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 1 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 6 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 9 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 12 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 1 | 156.4 KiB, 799 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 6 | 156.5 KiB, 603 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 9 | 156.5 KiB, 536 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 12 | 156.5 KiB, 207 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 1 | 156.5 KiB, 555 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 6 | 156.5 KiB, 459 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 9 | 156.5 KiB, 447 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 12 | 156.5 KiB, 288 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 1 | 156.5 KiB, 363 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 6 | 156.5 KiB, 315 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 9 | 156.5 KiB, 315 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 12 | 156.5 KiB, 602 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Rust's executables

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| rustc | 1 | 156.5 KiB, 144 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| rustc | 6 | 156.5 KiB, 135 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| rustc | 9 | 156.5 KiB, 135 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| rustc | 12 | 156.5 KiB, 209 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 1 | 156.5 KiB, 32274 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 6 | 156.5 KiB, 30960 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 9 | 156.5 KiB, 30728 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 12 | 156.5 KiB, 32360 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 1 | 156.5 KiB, 8327 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 6 | 156.5 KiB, 7884 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 9 | 156.5 KiB, 7811 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 12 | 156.5 KiB, 11109 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

Ours relative to zlib's peak: 4.01x at the median, and at most 4.01x.

### Intel Xeon 6975P-C (1 vCPU under EC2 Nitro), whole

Measured 2026-11-15, at `9e96979`, `decode-deflate` 0.0.4.

The size of each decoder's value: ours 168 B, zlib 112 B, zlib-ng 104 B,
libdeflate 8 B, zlib-rs 128 B, miniz_oxide 8 B, zune-inflate 11520 B,
yazi 7296 B.

#### Synthetic

| Input | Level | ours | zlib | zlib-ng | libdeflate | zlib-rs | miniz_oxide | zune-inflate | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| random | 1 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16388.0 KiB, 2 | 0.0 KiB, 0 |
| random | 6 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| random | 9 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| random | 12 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| zeros | 1 | 147.5 KiB, 23 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| zeros | 6 | 147.5 KiB, 23 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| zeros | 9 | 147.5 KiB, 23 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| zeros | 12 | 147.5 KiB, 283 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| pattern | 1 | 147.5 KiB, 23 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| pattern | 6 | 147.5 KiB, 23 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| pattern | 9 | 147.5 KiB, 23 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| pattern | 12 | 147.5 KiB, 283 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| far | 1 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| far | 6 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| far | 9 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |
| far | 12 | 129.1 KiB, 3 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16384.0 KiB, 2 | 0.0 KiB, 0 |

#### Silesia

| Input | Level | ours | zlib | zlib-ng | libdeflate | zlib-rs | miniz_oxide | zune-inflate | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| dickens | 1 | 156.5 KiB, 1731 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19908.0 KiB, 2 | 0.0 KiB, 0 |
| dickens | 6 | 156.5 KiB, 1527 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19907.1 KiB, 2 | 0.0 KiB, 0 |
| dickens | 9 | 156.5 KiB, 1515 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19907.1 KiB, 2 | 0.0 KiB, 0 |
| dickens | 12 | 156.5 KiB, 639 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19907.1 KiB, 2 | 0.0 KiB, 0 |
| mozilla | 1 | 156.5 KiB, 10485 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 100044.0 KiB, 2 | 0.0 KiB, 0 |
| mozilla | 6 | 156.5 KiB, 9989 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 100044.0 KiB, 2 | 0.0 KiB, 0 |
| mozilla | 9 | 156.5 KiB, 9960 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 100044.0 KiB, 2 | 0.0 KiB, 0 |
| mozilla | 12 | 156.5 KiB, 23342 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 100044.0 KiB, 2 | 0.0 KiB, 0 |
| mr | 1 | 156.5 KiB, 1576 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19473.8 KiB, 2 | 0.0 KiB, 0 |
| mr | 6 | 156.5 KiB, 1662 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19473.8 KiB, 2 | 0.0 KiB, 0 |
| mr | 9 | 156.5 KiB, 1670 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19473.8 KiB, 2 | 0.0 KiB, 0 |
| mr | 12 | 156.5 KiB, 1362 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19473.8 KiB, 2 | 0.0 KiB, 0 |
| nci | 1 | 156.5 KiB, 1671 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 65536.0 KiB, 2 | 0.0 KiB, 0 |
| nci | 6 | 156.5 KiB, 1311 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 65536.0 KiB, 2 | 0.0 KiB, 0 |
| nci | 9 | 156.5 KiB, 1311 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 65536.0 KiB, 2 | 0.0 KiB, 0 |
| nci | 12 | 156.5 KiB, 3853 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 65536.0 KiB, 2 | 0.0 KiB, 0 |
| ooffice | 1 | 156.5 KiB, 1635 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12016.0 KiB, 2 | 0.0 KiB, 0 |
| ooffice | 6 | 156.5 KiB, 1605 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12016.0 KiB, 2 | 0.0 KiB, 0 |
| ooffice | 9 | 156.5 KiB, 1597 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12016.0 KiB, 2 | 0.0 KiB, 0 |
| ooffice | 12 | 156.5 KiB, 1260 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12016.0 KiB, 2 | 0.0 KiB, 0 |
| osdb | 1 | 156.4 KiB, 2115 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19698.6 KiB, 2 | 0.0 KiB, 0 |
| osdb | 6 | 156.4 KiB, 1987 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19698.6 KiB, 2 | 0.0 KiB, 0 |
| osdb | 9 | 156.4 KiB, 1982 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19698.6 KiB, 2 | 0.0 KiB, 0 |
| osdb | 12 | 156.4 KiB, 423 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 19698.6 KiB, 2 | 0.0 KiB, 0 |
| reymont | 1 | 156.5 KiB, 915 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12943.8 KiB, 2 | 0.0 KiB, 0 |
| reymont | 6 | 156.5 KiB, 702 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12943.8 KiB, 2 | 0.0 KiB, 0 |
| reymont | 9 | 156.5 KiB, 699 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12943.8 KiB, 2 | 0.0 KiB, 0 |
| reymont | 12 | 155.5 KiB, 344 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 12943.8 KiB, 2 | 0.0 KiB, 0 |
| samba | 1 | 156.5 KiB, 2859 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 42204.0 KiB, 2 | 0.0 KiB, 0 |
| samba | 6 | 156.5 KiB, 2629 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 42204.0 KiB, 2 | 0.0 KiB, 0 |
| samba | 9 | 156.5 KiB, 2612 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 42204.0 KiB, 2 | 0.0 KiB, 0 |
| samba | 12 | 156.5 KiB, 8936 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 42204.0 KiB, 2 | 0.0 KiB, 0 |
| sao | 1 | 156.2 KiB, 3016 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 14164.0 KiB, 2 | 0.0 KiB, 0 |
| sao | 6 | 156.2 KiB, 3030 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 14164.0 KiB, 2 | 0.0 KiB, 0 |
| sao | 9 | 156.2 KiB, 3041 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 14164.0 KiB, 2 | 0.0 KiB, 0 |
| sao | 12 | 156.2 KiB, 303 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 14164.0 KiB, 2 | 0.0 KiB, 0 |
| webster | 1 | 156.5 KiB, 5559 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 80976.0 KiB, 2 | 0.0 KiB, 0 |
| webster | 6 | 156.5 KiB, 4983 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 80976.0 KiB, 2 | 0.0 KiB, 0 |
| webster | 9 | 156.5 KiB, 4959 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 80976.0 KiB, 2 | 0.0 KiB, 0 |
| webster | 12 | 156.5 KiB, 1933 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 80976.0 KiB, 2 | 0.0 KiB, 0 |
| xml | 1 | 156.5 KiB, 375 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 10440.0 KiB, 2 | 0.0 KiB, 0 |
| xml | 6 | 156.5 KiB, 291 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 10440.0 KiB, 2 | 0.0 KiB, 0 |
| xml | 9 | 156.5 KiB, 291 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 10440.0 KiB, 2 | 0.0 KiB, 0 |
| xml | 12 | 156.5 KiB, 705 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 10440.0 KiB, 2 | 0.0 KiB, 0 |
| x-ray | 1 | 156.2 KiB, 2677 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16551.3 KiB, 2 | 0.0 KiB, 0 |
| x-ray | 6 | 156.2 KiB, 3081 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16551.3 KiB, 2 | 0.0 KiB, 0 |
| x-ray | 9 | 156.2 KiB, 3081 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16551.3 KiB, 2 | 0.0 KiB, 0 |
| x-ray | 12 | 156.2 KiB, 409 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 16551.3 KiB, 2 | 0.0 KiB, 0 |

#### Large files

| Input | Level | ours | zlib | zlib-ng | libdeflate | zlib-rs | miniz_oxide | zune-inflate | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 1 | 156.5 KiB, 16587 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| enwik8 | 6 | 156.5 KiB, 15483 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| enwik8 | 9 | 156.5 KiB, 15447 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| enwik8 | 12 | 156.5 KiB, 47852 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| linux-6.12.tar | 1 | 156.5 KiB, 9603 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| linux-6.12.tar | 6 | 156.5 KiB, 8318 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| linux-6.12.tar | 9 | 156.5 KiB, 8258 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |
| linux-6.12.tar | 12 | 156.5 KiB, 32770 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 195316.0 KiB, 2 | 0.0 KiB, 0 |

#### Canterbury

| Input | Level | ours | zlib | zlib-ng | libdeflate | zlib-rs | miniz_oxide | zune-inflate | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 1 | 156.4 KiB, 38 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 297.1 KiB, 2 | 0.0 KiB, 0 |
| alice29.txt | 6 | 156.4 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 297.1 KiB, 2 | 0.0 KiB, 0 |
| alice29.txt | 9 | 156.4 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 297.1 KiB, 2 | 0.0 KiB, 0 |
| alice29.txt | 12 | 139.1 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 297.1 KiB, 2 | 0.0 KiB, 0 |
| asyoulik.txt | 1 | 139.1 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 244.5 KiB, 2 | 0.0 KiB, 0 |
| asyoulik.txt | 6 | 156.4 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 244.5 KiB, 2 | 0.0 KiB, 0 |
| asyoulik.txt | 9 | 156.5 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 244.5 KiB, 2 | 0.0 KiB, 0 |
| asyoulik.txt | 12 | 139.1 KiB, 27 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 244.5 KiB, 2 | 0.0 KiB, 0 |
| cp.html | 1 | 49.6 KiB, 13 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 48.1 KiB, 2 | 0.0 KiB, 0 |
| cp.html | 6 | 49.6 KiB, 13 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 48.1 KiB, 2 | 0.0 KiB, 0 |
| cp.html | 9 | 49.6 KiB, 13 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 48.1 KiB, 2 | 0.0 KiB, 0 |
| cp.html | 12 | 59.6 KiB, 25 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 48.1 KiB, 2 | 0.0 KiB, 0 |
| fields.c | 1 | 49.6 KiB, 13 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 21.8 KiB, 2 | 0.0 KiB, 0 |
| fields.c | 6 | 49.6 KiB, 12 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 21.8 KiB, 2 | 0.0 KiB, 0 |
| fields.c | 9 | 49.6 KiB, 12 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 21.8 KiB, 2 | 0.0 KiB, 0 |
| fields.c | 12 | 49.7 KiB, 13 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 21.8 KiB, 2 | 0.0 KiB, 0 |
| grammar.lsp | 1 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7.9 KiB, 2 | 0.0 KiB, 0 |
| grammar.lsp | 6 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7.9 KiB, 2 | 0.0 KiB, 0 |
| grammar.lsp | 9 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7.9 KiB, 2 | 0.0 KiB, 0 |
| grammar.lsp | 12 | 45.6 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7.9 KiB, 2 | 0.0 KiB, 0 |
| kennedy.xls | 1 | 156.2 KiB, 134 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 2011.2 KiB, 2 | 0.0 KiB, 0 |
| kennedy.xls | 6 | 156.2 KiB, 123 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 2011.2 KiB, 2 | 0.0 KiB, 0 |
| kennedy.xls | 9 | 156.2 KiB, 123 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 2011.2 KiB, 2 | 0.0 KiB, 0 |
| kennedy.xls | 12 | 156.2 KiB, 814 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 2011.2 KiB, 2 | 0.0 KiB, 0 |
| lcet10.txt | 1 | 156.5 KiB, 75 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 833.5 KiB, 2 | 0.0 KiB, 0 |
| lcet10.txt | 6 | 156.5 KiB, 63 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 833.5 KiB, 2 | 0.0 KiB, 0 |
| lcet10.txt | 9 | 156.5 KiB, 63 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 833.5 KiB, 2 | 0.0 KiB, 0 |
| lcet10.txt | 12 | 156.5 KiB, 75 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 833.5 KiB, 2 | 0.0 KiB, 0 |
| plrabn12.txt | 1 | 156.4 KiB, 99 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 941.1 KiB, 2 | 0.0 KiB, 0 |
| plrabn12.txt | 6 | 156.5 KiB, 87 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 941.1 KiB, 2 | 0.0 KiB, 0 |
| plrabn12.txt | 9 | 156.5 KiB, 87 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 941.1 KiB, 2 | 0.0 KiB, 0 |
| plrabn12.txt | 12 | 156.4 KiB, 51 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 941.1 KiB, 2 | 0.0 KiB, 0 |
| ptt5 | 1 | 156.5 KiB, 39 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1002.4 KiB, 2 | 0.0 KiB, 0 |
| ptt5 | 6 | 156.5 KiB, 39 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1002.4 KiB, 2 | 0.0 KiB, 0 |
| ptt5 | 9 | 156.5 KiB, 39 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1002.4 KiB, 2 | 0.0 KiB, 0 |
| ptt5 | 12 | 156.5 KiB, 202 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1002.4 KiB, 2 | 0.0 KiB, 0 |
| sum | 1 | 73.6 KiB, 13 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 74.7 KiB, 2 | 0.0 KiB, 0 |
| sum | 6 | 74.6 KiB, 14 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 74.7 KiB, 2 | 0.0 KiB, 0 |
| sum | 9 | 74.6 KiB, 14 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 74.7 KiB, 2 | 0.0 KiB, 0 |
| sum | 12 | 74.6 KiB, 38 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 74.7 KiB, 2 | 0.0 KiB, 0 |
| xargs.1 | 1 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 8.4 KiB, 2 | 0.0 KiB, 0 |
| xargs.1 | 6 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 8.4 KiB, 2 | 0.0 KiB, 0 |
| xargs.1 | 9 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 8.4 KiB, 2 | 0.0 KiB, 0 |
| xargs.1 | 12 | 45.5 KiB, 11 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 8.4 KiB, 2 | 0.0 KiB, 0 |
| E.coli | 1 | 156.4 KiB, 799 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 9059.9 KiB, 2 | 0.0 KiB, 0 |
| E.coli | 6 | 156.5 KiB, 603 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 9059.9 KiB, 2 | 0.0 KiB, 0 |
| E.coli | 9 | 156.5 KiB, 536 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 9059.9 KiB, 2 | 0.0 KiB, 0 |
| E.coli | 12 | 156.5 KiB, 207 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 9059.9 KiB, 2 | 0.0 KiB, 0 |
| bible.txt | 1 | 156.5 KiB, 555 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7905.1 KiB, 2 | 0.0 KiB, 0 |
| bible.txt | 6 | 156.5 KiB, 459 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7905.1 KiB, 2 | 0.0 KiB, 0 |
| bible.txt | 9 | 156.5 KiB, 447 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7905.1 KiB, 2 | 0.0 KiB, 0 |
| bible.txt | 12 | 156.5 KiB, 288 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 7905.1 KiB, 2 | 0.0 KiB, 0 |
| world192.txt | 1 | 156.5 KiB, 363 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 4830.9 KiB, 2 | 0.0 KiB, 0 |
| world192.txt | 6 | 156.5 KiB, 315 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 4830.9 KiB, 2 | 0.0 KiB, 0 |
| world192.txt | 9 | 156.5 KiB, 315 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 4830.9 KiB, 2 | 0.0 KiB, 0 |
| world192.txt | 12 | 156.5 KiB, 602 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 4830.9 KiB, 2 | 0.0 KiB, 0 |

#### Rust's executables

| Input | Level | ours | zlib | zlib-ng | libdeflate | zlib-rs | miniz_oxide | zune-inflate | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| rustc | 1 | 156.5 KiB, 144 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1262.4 KiB, 2 | 0.0 KiB, 0 |
| rustc | 6 | 156.5 KiB, 135 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1262.4 KiB, 2 | 0.0 KiB, 0 |
| rustc | 9 | 156.5 KiB, 135 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1262.4 KiB, 2 | 0.0 KiB, 0 |
| rustc | 12 | 156.5 KiB, 209 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 1262.4 KiB, 2 | 0.0 KiB, 0 |
| librustc_driver.so | 1 | 156.5 KiB, 32274 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 308356.0 KiB, 2 | 0.0 KiB, 0 |
| librustc_driver.so | 6 | 156.5 KiB, 30960 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 308356.0 KiB, 2 | 0.0 KiB, 0 |
| librustc_driver.so | 9 | 156.5 KiB, 30728 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 308356.0 KiB, 2 | 0.0 KiB, 0 |
| librustc_driver.so | 12 | 156.5 KiB, 32360 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 308356.0 KiB, 2 | 0.0 KiB, 0 |
| cargo | 1 | 156.5 KiB, 8327 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 83684.0 KiB, 2 | 0.0 KiB, 0 |
| cargo | 6 | 156.5 KiB, 7884 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 83684.0 KiB, 2 | 0.0 KiB, 0 |
| cargo | 9 | 156.5 KiB, 7811 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 83684.0 KiB, 2 | 0.0 KiB, 0 |
| cargo | 12 | 156.5 KiB, 11109 | 7.0 KiB, 1 | 41.1 KiB, 1 | 11.3 KiB, 1 | 46.4 KiB, 1 | 0.0 KiB, 0 | 83684.0 KiB, 2 | 0.0 KiB, 0 |

Ours relative to zlib's peak: 22.38x at the median, and at most 22.38x.

### Intel Xeon 6975P-C (1 vCPU under EC2 Nitro), in pieces of 4 KiB

Measured 2026-11-15, at `9e96979`, `decode-deflate` 0.0.4.

The size of each decoder's value: ours 168 B, zlib 112 B, zlib-ng 104 B,
zlib-rs 128 B, miniz_oxide 8 B, yazi 7296 B.

#### Synthetic

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| random | 1 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| random | 6 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| random | 9 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| random | 12 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 1 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 6 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 9 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 12 | 147.5 KiB, 283 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 1 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 6 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 9 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 12 | 147.5 KiB, 283 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 1 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 6 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 9 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 12 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Silesia

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| dickens | 1 | 156.5 KiB, 1731 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| dickens | 6 | 156.5 KiB, 1527 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| dickens | 9 | 156.5 KiB, 1515 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| dickens | 12 | 156.5 KiB, 639 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 1 | 156.5 KiB, 10485 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 6 | 156.5 KiB, 9989 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 9 | 156.5 KiB, 9960 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 12 | 156.5 KiB, 23342 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 1 | 156.5 KiB, 1576 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 6 | 156.5 KiB, 1662 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 9 | 156.5 KiB, 1670 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 12 | 156.5 KiB, 1362 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 1 | 156.5 KiB, 1671 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 6 | 156.5 KiB, 1311 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 9 | 156.5 KiB, 1311 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 12 | 156.5 KiB, 3853 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 1 | 156.5 KiB, 1635 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 6 | 156.5 KiB, 1605 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 9 | 156.5 KiB, 1597 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 12 | 156.5 KiB, 1260 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 1 | 156.4 KiB, 2115 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 6 | 156.4 KiB, 1987 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 9 | 156.4 KiB, 1982 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 12 | 156.4 KiB, 423 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 1 | 156.5 KiB, 915 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 6 | 156.5 KiB, 702 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 9 | 156.5 KiB, 699 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 12 | 155.5 KiB, 344 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 1 | 156.5 KiB, 2859 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 6 | 156.5 KiB, 2629 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 9 | 156.5 KiB, 2612 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 12 | 156.5 KiB, 8936 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 1 | 156.2 KiB, 3016 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 6 | 156.2 KiB, 3030 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 9 | 156.2 KiB, 3041 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 12 | 156.2 KiB, 303 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 1 | 156.5 KiB, 5559 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 6 | 156.5 KiB, 4983 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 9 | 156.5 KiB, 4959 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 12 | 156.5 KiB, 1933 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 1 | 156.5 KiB, 375 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 6 | 156.5 KiB, 291 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 9 | 156.5 KiB, 291 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 12 | 156.5 KiB, 705 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 1 | 156.2 KiB, 2677 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 6 | 156.2 KiB, 3081 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 9 | 156.2 KiB, 3081 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 12 | 156.2 KiB, 409 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Large files

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 1 | 156.5 KiB, 16587 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| enwik8 | 6 | 156.5 KiB, 15483 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| enwik8 | 9 | 156.5 KiB, 15447 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| enwik8 | 12 | 156.5 KiB, 47852 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 1 | 156.5 KiB, 9603 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 6 | 156.5 KiB, 8318 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 9 | 156.5 KiB, 8258 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 12 | 156.5 KiB, 32770 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Canterbury

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 1 | 156.4 KiB, 38 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| alice29.txt | 6 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| alice29.txt | 9 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| alice29.txt | 12 | 139.1 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 1 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 6 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 9 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 12 | 139.1 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 1 | 74.6 KiB, 14 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 6 | 74.6 KiB, 14 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 9 | 74.6 KiB, 14 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 12 | 91.9 KiB, 26 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 1 | 49.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 6 | 49.6 KiB, 12 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 9 | 49.6 KiB, 12 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 12 | 49.7 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 1 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 6 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 9 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 12 | 45.6 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 1 | 156.2 KiB, 134 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 6 | 156.2 KiB, 123 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 9 | 156.2 KiB, 123 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 12 | 156.2 KiB, 814 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 1 | 156.5 KiB, 75 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 6 | 156.5 KiB, 63 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 9 | 156.5 KiB, 63 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 12 | 156.5 KiB, 75 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 1 | 156.4 KiB, 99 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 6 | 156.5 KiB, 87 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 9 | 156.5 KiB, 87 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 12 | 156.4 KiB, 51 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 1 | 156.5 KiB, 39 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 6 | 156.5 KiB, 39 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 9 | 156.5 KiB, 39 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 12 | 156.5 KiB, 202 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 1 | 73.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 6 | 139.1 KiB, 15 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 9 | 139.1 KiB, 15 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 12 | 92.0 KiB, 38 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 1 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 6 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 9 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 12 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 1 | 156.5 KiB, 799 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 6 | 156.5 KiB, 603 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 9 | 156.5 KiB, 536 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 12 | 156.5 KiB, 207 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 1 | 156.5 KiB, 555 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 6 | 156.5 KiB, 459 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 9 | 156.5 KiB, 447 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 12 | 156.5 KiB, 288 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 1 | 156.5 KiB, 363 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 6 | 156.5 KiB, 315 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 9 | 156.5 KiB, 315 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 12 | 156.5 KiB, 602 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Rust's executables

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| rustc | 1 | 156.5 KiB, 144 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| rustc | 6 | 156.5 KiB, 135 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| rustc | 9 | 156.5 KiB, 135 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| rustc | 12 | 156.5 KiB, 209 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 1 | 156.5 KiB, 32274 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 6 | 156.5 KiB, 30960 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 9 | 156.5 KiB, 30728 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 12 | 156.5 KiB, 32360 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 1 | 156.5 KiB, 8327 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 6 | 156.5 KiB, 7884 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 9 | 156.5 KiB, 7811 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 12 | 156.5 KiB, 11109 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

Ours relative to zlib's peak: 4.01x at the median, and at most 4.01x.

### Intel Xeon 6975P-C (1 vCPU under EC2 Nitro), in pieces of 64 KiB

Measured 2026-11-15, at `9e96979`, `decode-deflate` 0.0.4.

The size of each decoder's value: ours 168 B, zlib 112 B, zlib-ng 104 B,
zlib-rs 128 B, miniz_oxide 8 B, yazi 7296 B.

#### Synthetic

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| random | 1 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| random | 6 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| random | 9 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| random | 12 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 1 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 6 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 9 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| zeros | 12 | 147.5 KiB, 283 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 1 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 6 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 9 | 147.5 KiB, 23 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| pattern | 12 | 147.5 KiB, 283 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 1 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 6 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 9 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| far | 12 | 129.1 KiB, 3 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Silesia

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| dickens | 1 | 156.5 KiB, 1731 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| dickens | 6 | 156.5 KiB, 1527 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| dickens | 9 | 156.5 KiB, 1515 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| dickens | 12 | 156.5 KiB, 639 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 1 | 156.5 KiB, 10485 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 6 | 156.5 KiB, 9989 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 9 | 156.5 KiB, 9960 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mozilla | 12 | 156.5 KiB, 23342 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 1 | 156.5 KiB, 1576 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 6 | 156.5 KiB, 1662 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 9 | 156.5 KiB, 1670 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| mr | 12 | 156.5 KiB, 1362 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 1 | 156.5 KiB, 1671 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 6 | 156.5 KiB, 1311 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 9 | 156.5 KiB, 1311 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| nci | 12 | 156.5 KiB, 3853 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 1 | 156.5 KiB, 1635 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 6 | 156.5 KiB, 1605 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 9 | 156.5 KiB, 1597 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ooffice | 12 | 156.5 KiB, 1260 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 1 | 156.4 KiB, 2115 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 6 | 156.4 KiB, 1987 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 9 | 156.4 KiB, 1982 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| osdb | 12 | 156.4 KiB, 423 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 1 | 156.5 KiB, 915 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 6 | 156.5 KiB, 702 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 9 | 156.5 KiB, 699 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| reymont | 12 | 155.5 KiB, 344 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 1 | 156.5 KiB, 2859 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 6 | 156.5 KiB, 2629 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 9 | 156.5 KiB, 2612 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| samba | 12 | 156.5 KiB, 8936 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 1 | 156.2 KiB, 3016 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 6 | 156.2 KiB, 3030 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 9 | 156.2 KiB, 3041 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sao | 12 | 156.2 KiB, 303 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 1 | 156.5 KiB, 5559 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 6 | 156.5 KiB, 4983 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 9 | 156.5 KiB, 4959 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| webster | 12 | 156.5 KiB, 1933 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 1 | 156.5 KiB, 375 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 6 | 156.5 KiB, 291 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 9 | 156.5 KiB, 291 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xml | 12 | 156.5 KiB, 705 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 1 | 156.2 KiB, 2677 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 6 | 156.2 KiB, 3081 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 9 | 156.2 KiB, 3081 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| x-ray | 12 | 156.2 KiB, 409 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Large files

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| enwik8 | 1 | 156.5 KiB, 16587 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| enwik8 | 6 | 156.5 KiB, 15483 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| enwik8 | 9 | 156.5 KiB, 15447 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| enwik8 | 12 | 156.5 KiB, 47852 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 1 | 156.5 KiB, 9603 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 6 | 156.5 KiB, 8318 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 9 | 156.5 KiB, 8258 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| linux-6.12.tar | 12 | 156.5 KiB, 32770 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Canterbury

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| alice29.txt | 1 | 156.4 KiB, 38 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| alice29.txt | 6 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| alice29.txt | 9 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| alice29.txt | 12 | 139.1 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 1 | 139.1 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 6 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 9 | 156.4 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| asyoulik.txt | 12 | 139.1 KiB, 27 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 1 | 49.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 6 | 49.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 9 | 49.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cp.html | 12 | 59.6 KiB, 25 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 1 | 49.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 6 | 49.6 KiB, 12 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 9 | 49.6 KiB, 12 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| fields.c | 12 | 49.7 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 1 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 6 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 9 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| grammar.lsp | 12 | 45.6 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 1 | 156.2 KiB, 134 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 6 | 156.2 KiB, 123 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 9 | 156.2 KiB, 123 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| kennedy.xls | 12 | 156.2 KiB, 814 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 1 | 156.5 KiB, 75 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 6 | 156.5 KiB, 63 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 9 | 156.5 KiB, 63 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| lcet10.txt | 12 | 156.5 KiB, 75 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 1 | 156.4 KiB, 99 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 6 | 156.5 KiB, 87 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 9 | 156.5 KiB, 87 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| plrabn12.txt | 12 | 156.4 KiB, 51 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 1 | 156.5 KiB, 39 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 6 | 156.5 KiB, 39 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 9 | 156.5 KiB, 39 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| ptt5 | 12 | 156.5 KiB, 202 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 1 | 73.6 KiB, 13 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 6 | 74.6 KiB, 14 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 9 | 74.6 KiB, 14 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| sum | 12 | 74.6 KiB, 38 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 1 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 6 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 9 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| xargs.1 | 12 | 45.5 KiB, 11 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 1 | 156.5 KiB, 799 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 6 | 156.5 KiB, 603 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 9 | 156.5 KiB, 536 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| E.coli | 12 | 156.5 KiB, 207 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 1 | 156.5 KiB, 555 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 6 | 156.5 KiB, 459 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 9 | 156.5 KiB, 447 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| bible.txt | 12 | 156.5 KiB, 288 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 1 | 156.5 KiB, 363 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 6 | 156.5 KiB, 315 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 9 | 156.5 KiB, 315 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| world192.txt | 12 | 156.5 KiB, 602 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

#### Rust's executables

| Input | Level | ours | zlib | zlib-ng | zlib-rs | miniz_oxide | yazi |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| rustc | 1 | 156.5 KiB, 144 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| rustc | 6 | 156.5 KiB, 135 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| rustc | 9 | 156.5 KiB, 135 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| rustc | 12 | 156.5 KiB, 209 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 1 | 156.5 KiB, 32274 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 6 | 156.5 KiB, 30960 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 9 | 156.5 KiB, 30728 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| librustc_driver.so | 12 | 156.5 KiB, 32360 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 1 | 156.5 KiB, 8327 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 6 | 156.5 KiB, 7884 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 9 | 156.5 KiB, 7811 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |
| cargo | 12 | 156.5 KiB, 11109 | 39.0 KiB, 2 | 41.1 KiB, 1 | 46.4 KiB, 1 | 42.3 KiB, 1 | 0.0 KiB, 0 |

Ours relative to zlib's peak: 4.01x at the median, and at most 4.01x.

<!-- filer-bench: end -->

## Checkpoints

The time to take a checkpoint and to resume from one, and the bytes each
holds.

## Code size

The bytes of `.text` the decoder adds to a minimal `#![no_std]` program
that decodes a stream with it, from construction to `finish`, built
portable with fat LTO; read-only data, such as tables, is not counted.

<!-- filer-bench: size -->

| Target | `opt-level` 3 | `opt-level` "s" |
| --- | ---: | ---: |
| aarch64-apple-darwin | 6772 | 6416 |
| aarch64-unknown-linux-gnu | 8728 | 7076 |
| x86_64-unknown-linux-gnu | 11876 | 8860 |

On aarch64-apple-darwin, measured 2026-11-07, at `86d2427`,
`decode-deflate` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On aarch64-unknown-linux-gnu, measured 2026-11-13, at `de4f027`,
`decode-deflate` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On x86_64-unknown-linux-gnu, measured 2026-11-13, at `de4f027`,
`decode-deflate` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

<!-- filer-bench: end -->
