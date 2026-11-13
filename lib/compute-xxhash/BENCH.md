# compute-xxhash benchmarks

The figures of `compute-xxhash`'s xxHash hashes, alone and against the main
alternatives, measured by the protocol of this repository's
[`bench/METHOD.md`](../../bench/METHOD.md): the same inputs, machines,
and builds for every implementation, each figure a median with its 95%
confidence interval, and every result checked against the others'.

## Counterparts

| Implementation | Crate | Language |
| --- | --- | --- |
| xxHash | `xxhash-c-sys` | C |
| xxhash-rust | `xxhash-rust` | Rust |
| twox-hash | `twox-hash` | Rust |

## Inputs

Buffers of random bytes, of 16 B, 256 B, 4 KiB, 64 KiB, and 1 MiB, fed
whole, by one update, and in pieces of 7 bytes, for XXH32 and XXH64.

## Machines

64-bit x86 on a current AMD core and a current Intel core, each a
cloud instance's own, and on an older AMD core, a VM's; and 64-bit ARM
on Apple M4 and on a Neoverse server.

Measured so far, each under Linux 7.0: AMD EPYC 9R45 (Zen 5), Intel
Xeon 6975P-C (Granite Rapids), and ARM Neoverse V3 (AWS Graviton5),
each 1 vCPU under EC2 Nitro, on a physical core of its own, by Rust
1.99.1, 2026-11-13; and AMD EPYC 7551P (Zen 1), 4 vCPUs under KVM, by
Rust 1.97.0, 2026-11-06.

## Throughput

GB/s, each the median of five or more processes with its 95%
confidence interval: a row per size and way of feeding, a column per
implementation; then each counterpart's speed relative to ours, the
geometric mean over sizes. The counterparts pick the CPU's
instructions at run time.

### Zen 5, ours portable, as by default

#### XXH32

| Bytes | Fed | ours | xxHash | xxhash-rust | twox-hash |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.90 ± 0.3% | 0.70 ± 0.3% | 2.04 ± 0.7% | 2.02 ± 0.2% |
| 256 | whole | 10.26 ± 0.1% | 1.85 ± 0.2% | 10.64 ± 0.4% | 10.56 ± 0.3% |
| 4096 | whole | 12.01 ± 0.6% | 2.07 ± 0.2% | 12.12 ± 0.9% | 12.23 ± 0.9% |
| 65536 | whole | 12.65 ± 0.1% | 2.08 ± 0.2% | 12.62 ± 0.4% | 12.70 ± 0.3% |
| 1048576 | whole | 12.64 ± 0.1% | 2.06 ± 0.2% | 12.59 ± 0.1% | 12.67 ± 0.1% |
| 16 | by 7 | 0.86 ± 0.8% | 0.56 ± 0.5% | 0.84 ± 0.6% | 0.83 ± 0.6% |
| 256 | by 7 | 1.47 ± 0.2% | 0.90 ± 0.6% | 1.43 ± 0.2% | 1.55 ± 0.1% |
| 4096 | by 7 | 1.53 ± 0.0% | 0.93 ± 1.0% | 1.53 ± 0.2% | 1.62 ± 0.5% |
| 65536 | by 7 | 1.54 ± 0.1% | 0.93 ± 1.0% | 1.55 ± 0.1% | 1.63 ± 0.4% |
| 1048576 | by 7 | 1.54 ± 0.1% | 0.93 ± 1.0% | 1.55 ± 0.1% | 1.63 ± 0.2% |

Each counterpart's speed relative to ours: whole: xxHash 0.20x,
xxhash-rust 1.02x, twox-hash 1.02x; by 7: xxHash 0.62x, xxhash-rust
0.99x, twox-hash 1.04x.

#### XXH64

| Bytes | Fed | ours | xxHash | xxhash-rust | twox-hash |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.89 ± 1.1% | 1.41 ± 0.5% | 1.96 ± 0.2% | 1.97 ± 0.5% |
| 256 | whole | 14.31 ± 0.6% | 12.03 ± 0.0% | 14.46 ± 0.4% | 14.48 ± 0.3% |
| 4096 | whole | 23.77 ± 0.2% | 23.40 ± 0.1% | 23.66 ± 0.2% | 23.97 ± 0.2% |
| 65536 | whole | 25.06 ± 0.2% | 24.99 ± 0.2% | 24.67 ± 0.1% | 25.28 ± 0.2% |
| 1048576 | whole | 25.12 ± 0.2% | 25.13 ± 0.2% | 24.93 ± 0.1% | 25.27 ± 0.3% |
| 16 | by 7 | 0.82 ± 0.2% | 0.76 ± 0.5% | 0.81 ± 0.8% | 0.80 ± 0.3% |
| 256 | by 7 | 1.94 ± 0.2% | 2.18 ± 0.3% | 2.00 ± 0.1% | 1.96 ± 0.1% |
| 4096 | by 7 | 2.13 ± 0.1% | 2.26 ± 1.0% | 2.09 ± 0.1% | 2.16 ± 0.0% |
| 65536 | by 7 | 2.15 ± 0.1% | 2.32 ± 0.8% | 2.10 ± 0.1% | 2.18 ± 0.1% |
| 1048576 | by 7 | 2.15 ± 0.1% | 2.30 ± 0.9% | 2.10 ± 0.2% | 2.16 ± 0.8% |

Each counterpart's speed relative to ours: whole: xxHash 0.91x,
xxhash-rust 1.00x, twox-hash 1.02x; by 7: xxHash 1.05x, xxhash-rust
0.99x, twox-hash 1.01x.

### Granite Rapids, ours portable, as by default

#### XXH32

| Bytes | Fed | ours | xxHash | xxhash-rust | twox-hash |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.58 ± 0.1% | 0.66 ± 1.1% | 1.57 ± 0.5% | 1.58 ± 1.3% |
| 256 | whole | 7.57 ± 0.2% | 2.74 ± 0.6% | 7.52 ± 0.1% | 7.58 ± 0.1% |
| 4096 | whole | 7.64 ± 0.1% | 3.43 ± 0.1% | 7.62 ± 0.1% | 7.64 ± 0.1% |
| 65536 | whole | 7.71 ± 0.1% | 3.48 ± 0.1% | 7.71 ± 0.1% | 7.71 ± 0.1% |
| 1048576 | whole | 7.70 ± 0.1% | 3.46 ± 0.1% | 7.70 ± 0.1% | 7.70 ± 0.1% |
| 16 | by 7 | 0.67 ± 0.2% | 0.52 ± 0.1% | 0.65 ± 0.7% | 0.68 ± 0.4% |
| 256 | by 7 | 1.24 ± 0.2% | 0.89 ± 0.8% | 1.01 ± 0.4% | 1.26 ± 0.1% |
| 4096 | by 7 | 1.34 ± 0.2% | 0.94 ± 0.8% | 1.02 ± 0.9% | 1.35 ± 0.2% |
| 65536 | by 7 | 1.35 ± 0.0% | 0.95 ± 0.7% | 1.05 ± 0.9% | 1.35 ± 0.1% |
| 1048576 | by 7 | 1.35 ± 0.0% | 0.95 ± 0.4% | 1.05 ± 0.9% | 1.36 ± 0.0% |

Each counterpart's speed relative to ours: whole: xxHash 0.42x,
xxhash-rust 1.00x, twox-hash 1.01x; by 7: xxHash 0.72x, xxhash-rust
0.81x, twox-hash 1.01x.

#### XXH64

| Bytes | Fed | ours | xxHash | xxhash-rust | twox-hash |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.45 ± 2.3% | 0.64 ± 0.5% | 1.40 ± 2.3% | 1.43 ± 2.0% |
| 256 | whole | 12.11 ± 0.2% | 8.30 ± 0.6% | 12.43 ± 0.2% | 12.59 ± 0.1% |
| 4096 | whole | 15.05 ± 0.0% | 14.54 ± 0.2% | 14.97 ± 0.1% | 14.79 ± 0.1% |
| 65536 | whole | 15.39 ± 0.0% | 15.34 ± 0.1% | 15.40 ± 0.0% | 15.40 ± 0.0% |
| 1048576 | whole | 15.38 ± 0.1% | 15.37 ± 0.1% | 15.39 ± 0.1% | 15.38 ± 0.2% |
| 16 | by 7 | 0.65 ± 0.7% | 0.51 ± 0.5% | 0.63 ± 0.9% | 0.64 ± 0.7% |
| 256 | by 7 | 1.51 ± 0.3% | 1.48 ± 0.2% | 1.48 ± 0.9% | 1.54 ± 0.8% |
| 4096 | by 7 | 1.75 ± 0.8% | 1.57 ± 0.3% | 1.68 ± 0.6% | 1.85 ± 1.1% |
| 65536 | by 7 | 1.81 ± 0.6% | 1.56 ± 0.3% | 1.68 ± 0.9% | 1.87 ± 0.5% |
| 1048576 | by 7 | 1.76 ± 0.1% | 1.56 ± 0.1% | 1.72 ± 0.5% | 1.85 ± 0.5% |

Each counterpart's speed relative to ours: whole: xxHash 0.78x,
xxhash-rust 1.01x, twox-hash 1.01x; by 7: xxHash 0.88x, xxhash-rust
0.96x, twox-hash 1.03x.

### Neoverse V3, ours portable, as by default

#### XXH32

| Bytes | Fed | ours | xxHash | xxhash-rust | twox-hash |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.88 ± 1.7% | 2.13 ± 1.5% | 1.93 ± 1.9% | 1.95 ± 2.3% |
| 256 | whole | 7.65 ± 0.6% | 7.72 ± 2.1% | 8.98 ± 0.3% | 8.71 ± 0.2% |
| 4096 | whole | 9.26 ± 0.1% | 8.98 ± 0.8% | 4.47 ± 0.0% | 4.44 ± 0.0% |
| 65536 | whole | 9.44 ± 0.1% | 8.31 ± 0.7% | 4.27 ± 0.9% | 4.25 ± 0.6% |
| 1048576 | whole | 9.45 ± 0.4% | 8.22 ± 0.9% | 4.36 ± 0.0% | 4.35 ± 0.0% |
| 16 | by 7 | 0.62 ± 2.8% | 0.56 ± 0.5% | 0.53 ± 2.7% | 0.47 ± 13.8% |
| 256 | by 7 | 0.99 ± 0.2% | 0.74 ± 0.7% | 0.84 ± 0.2% | 0.74 ± 1.8% |
| 4096 | by 7 | 1.08 ± 3.7% | 0.75 ± 1.1% | 0.87 ± 0.0% | 0.75 ± 1.9% |
| 65536 | by 7 | 1.09 ± 4.4% | 0.75 ± 1.0% | 0.87 ± 0.0% | 0.76 ± 2.0% |
| 1048576 | by 7 | 1.09 ± 3.3% | 0.75 ± 0.8% | 0.87 ± 0.0% | 0.76 ± 2.1% |

Each counterpart's speed relative to ours: whole: xxHash 0.97x,
xxhash-rust 0.66x, twox-hash 0.65x; by 7: xxHash 0.74x, xxhash-rust
0.83x, twox-hash 0.74x.

#### XXH64

| Bytes | Fed | ours | xxHash | xxhash-rust | twox-hash |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.22 ± 2.8% | 1.02 ± 2.5% | 1.19 ± 6.5% | 1.08 ± 0.2% |
| 256 | whole | 11.85 ± 1.0% | 11.51 ± 1.0% | 12.29 ± 1.0% | 12.15 ± 0.7% |
| 4096 | whole | 17.39 ± 0.9% | 16.56 ± 0.8% | 17.72 ± 0.6% | 18.40 ± 1.1% |
| 65536 | whole | 16.51 ± 1.7% | 17.69 ± 1.6% | 17.81 ± 3.3% | 17.49 ± 2.7% |
| 1048576 | whole | 23.75 ± 2.5% | 21.11 ± 2.4% | 22.21 ± 3.2% | 22.54 ± 3.1% |
| 16 | by 7 | 0.80 ± 7.1% | 0.71 ± 1.8% | 0.75 ± 4.5% | 0.84 ± 7.9% |
| 256 | by 7 | 1.14 ± 0.3% | 0.91 ± 1.1% | 1.23 ± 2.5% | 0.89 ± 0.5% |
| 4096 | by 7 | 1.56 ± 0.5% | 0.92 ± 0.0% | 1.30 ± 0.1% | 0.93 ± 0.5% |
| 65536 | by 7 | 1.51 ± 5.7% | 0.93 ± 0.4% | 1.30 ± 1.3% | 0.93 ± 0.4% |
| 1048576 | by 7 | 1.68 ± 6.5% | 0.92 ± 0.3% | 1.30 ± 0.2% | 0.93 ± 0.7% |

Each counterpart's speed relative to ours: whole: xxHash 0.94x,
xxhash-rust 1.00x, twox-hash 0.99x; by 7: xxHash 0.67x, xxhash-rust
0.89x, twox-hash 0.70x.

### Zen 1, ours portable, as by default

#### XXH32

| Bytes | Fed | ours | xxHash | xxhash-rust | twox-hash |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 0.42 ± 0.3% | 0.35 ± 0.3% | 0.42 ± 0.3% | 0.43 ± 0.4% |
| 256 | whole | 2.98 ± 0.3% | 1.65 ± 0.2% | 2.87 ± 0.4% | 2.88 ± 0.2% |
| 4096 | whole | 4.72 ± 0.4% | 2.14 ± 0.2% | 4.24 ± 0.3% | 4.48 ± 0.4% |
| 65536 | whole | 4.94 ± 0.2% | 2.18 ± 0.1% | 4.41 ± 0.1% | 4.66 ± 0.1% |
| 1048576 | whole | 4.94 ± 0.6% | 2.17 ± 0.4% | 4.40 ± 0.5% | 4.66 ± 0.6% |
| 16 | by 7 | 0.26 ± 0.6% | 0.22 ± 2.5% | 0.25 ± 0.3% | 0.25 ± 0.5% |
| 256 | by 7 | 0.51 ± 0.3% | 0.44 ± 0.7% | 0.49 ± 0.6% | 0.50 ± 0.4% |
| 4096 | by 7 | 0.56 ± 0.4% | 0.46 ± 1.0% | 0.53 ± 0.5% | 0.53 ± 0.9% |
| 65536 | by 7 | 0.55 ± 0.9% | 0.47 ± 1.0% | 0.52 ± 0.8% | 0.54 ± 0.7% |
| 1048576 | by 7 | 0.55 ± 0.7% | 0.46 ± 0.6% | 0.52 ± 0.9% | 0.53 ± 0.6% |

Each counterpart's speed relative to ours: whole: xxHash 0.53x,
xxhash-rust 0.93x, twox-hash 0.96x; by 7: xxHash 0.84x, xxhash-rust
0.95x, twox-hash 0.96x.

#### XXH64

| Bytes | Fed | ours | xxHash | xxhash-rust | twox-hash |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 0.36 ± 0.5% | 0.33 ± 0.5% | 0.37 ± 0.5% | 0.37 ± 0.9% |
| 256 | whole | 3.52 ± 0.6% | 3.30 ± 0.6% | 3.49 ± 0.3% | 3.61 ± 0.4% |
| 4096 | whole | 8.72 ± 0.4% | 8.32 ± 0.3% | 8.45 ± 0.3% | 8.83 ± 0.2% |
| 65536 | whole | 9.55 ± 0.7% | 9.33 ± 0.3% | 9.37 ± 0.7% | 9.81 ± 0.3% |
| 1048576 | whole | 9.58 ± 0.3% | 9.35 ± 0.5% | 9.45 ± 0.8% | 9.79 ± 0.1% |
| 16 | by 7 | 0.27 ± 0.7% | 0.23 ± 0.9% | 0.25 ± 0.7% | 0.26 ± 0.5% |
| 256 | by 7 | 0.57 ± 0.6% | 0.57 ± 0.2% | 0.54 ± 0.3% | 0.55 ± 0.5% |
| 4096 | by 7 | 0.65 ± 0.2% | 0.66 ± 0.4% | 0.60 ± 0.4% | 0.62 ± 1.5% |
| 65536 | by 7 | 0.65 ± 0.5% | 0.67 ± 0.3% | 0.61 ± 0.3% | 0.62 ± 0.7% |
| 1048576 | by 7 | 0.65 ± 0.2% | 0.67 ± 0.3% | 0.61 ± 0.3% | 0.62 ± 2.3% |

Each counterpart's speed relative to ours: whole: xxHash 0.95x,
xxhash-rust 0.99x, twox-hash 1.03x; by 7: xxHash 0.99x, xxhash-rust
0.93x, twox-hash 0.96x.

## Code size

The bytes of `.text` each algorithm adds to a minimal `#![no_std]`
program that computes a checksum or hash with it, built portable with
fat LTO; read-only data, such as tables, is not counted.

<!-- filer-bench: size -->

| Algorithm | Target | `opt-level` 3 | `opt-level` "s" |
| --- | --- | ---: | ---: |
| xxh32 | aarch64-apple-darwin | 500 | 400 |
| xxh32 | aarch64-unknown-linux-gnu | 592 | 460 |
| xxh32 | x86_64-unknown-linux-gnu | 630 | 355 |
| xxh64 | aarch64-apple-darwin | 632 | 552 |
| xxh64 | aarch64-unknown-linux-gnu | 636 | 560 |
| xxh64 | x86_64-unknown-linux-gnu | 875 | 551 |

On aarch64-apple-darwin, measured 2026-11-07, at `86d2427`,
`compute-xxhash` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On aarch64-unknown-linux-gnu, measured 2026-11-13, at `de4f027`,
`compute-xxhash` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On x86_64-unknown-linux-gnu, measured 2026-11-13, at `de4f027`,
`compute-xxhash` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

<!-- filer-bench: end -->
