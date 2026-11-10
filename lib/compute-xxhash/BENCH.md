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

Measured so far, each under Linux 7.0, by Rust 1.97.0: AMD EPYC 9R45
(Zen 5) and Intel Xeon 6975P-C (Granite Rapids), each 1 vCPU under EC2
Nitro, on a physical core of its own, 2026-11-10; and AMD EPYC 7551P
(Zen 1), 4 vCPUs under KVM, 2026-11-06.

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
| 16 | whole | 1.99 ± 1.7% | 0.71 ± 0.5% | 2.16 ± 1.6% | 2.15 ± 1.3% |
| 256 | whole | 10.47 ± 0.1% | 1.86 ± 0.9% | 10.61 ± 0.1% | 10.74 ± 0.1% |
| 4096 | whole | 12.02 ± 0.9% | 2.06 ± 0.2% | 12.21 ± 0.9% | 12.29 ± 0.7% |
| 65536 | whole | 12.65 ± 0.1% | 2.08 ± 0.1% | 12.61 ± 0.1% | 12.64 ± 0.3% |
| 1048576 | whole | 12.67 ± 0.1% | 2.07 ± 0.1% | 12.63 ± 0.1% | 12.70 ± 0.2% |
| 16 | by 7 | 0.87 ± 0.2% | 0.57 ± 0.5% | 0.85 ± 0.7% | 0.85 ± 0.1% |
| 256 | by 7 | 1.47 ± 0.1% | 0.90 ± 1.0% | 1.43 ± 0.4% | 1.55 ± 0.2% |
| 4096 | by 7 | 1.53 ± 0.1% | 0.91 ± 0.9% | 1.55 ± 0.2% | 1.63 ± 0.6% |
| 65536 | by 7 | 1.54 ± 0.1% | 0.93 ± 0.6% | 1.55 ± 0.2% | 1.63 ± 0.6% |
| 1048576 | by 7 | 1.54 ± 0.1% | 0.93 ± 0.6% | 1.56 ± 0.2% | 1.63 ± 0.7% |

Each counterpart's speed relative to ours: whole: xxHash 0.20x,
xxhash-rust 1.02x, twox-hash 1.03x; by 7: xxHash 0.61x, xxhash-rust
1.00x, twox-hash 1.04x.

#### XXH64

| Bytes | Fed | ours | xxHash | xxhash-rust | twox-hash |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.99 ± 0.5% | 1.46 ± 0.5% | 2.07 ± 0.8% | 2.02 ± 0.7% |
| 256 | whole | 14.55 ± 2.5% | 12.15 ± 0.1% | 14.80 ± 0.1% | 14.68 ± 0.1% |
| 4096 | whole | 23.82 ± 0.0% | 23.47 ± 0.7% | 23.74 ± 0.9% | 24.02 ± 0.0% |
| 65536 | whole | 25.06 ± 0.1% | 25.01 ± 0.0% | 25.06 ± 0.2% | 25.32 ± 0.1% |
| 1048576 | whole | 25.15 ± 0.3% | 25.13 ± 0.2% | 25.12 ± 0.3% | 25.35 ± 0.2% |
| 16 | by 7 | 0.83 ± 0.5% | 0.77 ± 0.4% | 0.84 ± 0.8% | 0.82 ± 0.4% |
| 256 | by 7 | 1.94 ± 0.1% | 2.19 ± 0.2% | 1.83 ± 0.3% | 1.96 ± 0.2% |
| 4096 | by 7 | 2.13 ± 0.1% | 2.44 ± 0.8% | 2.08 ± 0.2% | 2.17 ± 0.1% |
| 65536 | by 7 | 2.14 ± 0.0% | 2.46 ± 0.1% | 2.10 ± 0.1% | 2.18 ± 0.6% |
| 1048576 | by 7 | 2.15 ± 0.0% | 2.46 ± 1.0% | 2.11 ± 0.2% | 2.19 ± 0.7% |

Each counterpart's speed relative to ours: whole: xxHash 0.91x,
xxhash-rust 1.01x, twox-hash 1.01x; by 7: xxHash 1.10x, xxhash-rust
0.98x, twox-hash 1.01x.

### Granite Rapids, ours portable, as by default

#### XXH32

| Bytes | Fed | ours | xxHash | xxhash-rust | twox-hash |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.54 ± 0.1% | 0.66 ± 1.3% | 1.54 ± 2.6% | 1.58 ± 0.1% |
| 256 | whole | 7.57 ± 0.2% | 2.74 ± 0.5% | 7.56 ± 0.2% | 7.57 ± 0.2% |
| 4096 | whole | 7.63 ± 0.0% | 3.43 ± 0.1% | 7.64 ± 0.1% | 7.65 ± 0.1% |
| 65536 | whole | 7.68 ± 0.4% | 3.47 ± 0.4% | 7.68 ± 0.4% | 7.69 ± 0.5% |
| 1048576 | whole | 7.67 ± 0.4% | 3.45 ± 0.5% | 7.68 ± 0.4% | 7.67 ± 0.5% |
| 16 | by 7 | 0.65 ± 1.6% | 0.52 ± 0.2% | 0.64 ± 2.5% | 0.66 ± 1.6% |
| 256 | by 7 | 1.23 ± 0.5% | 0.90 ± 0.4% | 1.24 ± 0.4% | 1.26 ± 0.4% |
| 4096 | by 7 | 1.34 ± 0.1% | 0.94 ± 0.2% | 1.28 ± 0.9% | 1.34 ± 0.2% |
| 65536 | by 7 | 1.35 ± 0.1% | 0.94 ± 0.7% | 1.29 ± 1.9% | 1.35 ± 0.2% |
| 1048576 | by 7 | 1.35 ± 0.1% | 0.95 ± 0.7% | 1.30 ± 0.9% | 1.35 ± 0.1% |

Each counterpart's speed relative to ours: whole: xxHash 0.43x,
xxhash-rust 1.00x, twox-hash 1.01x; by 7: xxHash 0.73x, xxhash-rust
0.98x, twox-hash 1.01x.

#### XXH64

| Bytes | Fed | ours | xxHash | xxhash-rust | twox-hash |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.40 ± 1.2% | 0.64 ± 0.7% | 1.39 ± 2.1% | 1.40 ± 1.7% |
| 256 | whole | 12.17 ± 0.2% | 8.33 ± 0.4% | 12.45 ± 0.3% | 12.55 ± 0.2% |
| 4096 | whole | 14.88 ± 0.2% | 14.51 ± 0.1% | 15.07 ± 0.1% | 14.66 ± 0.1% |
| 65536 | whole | 15.33 ± 0.2% | 15.32 ± 0.0% | 15.36 ± 0.1% | 15.36 ± 0.1% |
| 1048576 | whole | 15.36 ± 0.1% | 15.38 ± 0.1% | 15.37 ± 0.1% | 15.37 ± 0.1% |
| 16 | by 7 | 0.65 ± 0.3% | 0.51 ± 1.2% | 0.62 ± 1.2% | 0.63 ± 1.3% |
| 256 | by 7 | 1.54 ± 0.8% | 1.50 ± 0.2% | 1.46 ± 0.4% | 1.55 ± 0.5% |
| 4096 | by 7 | 1.85 ± 0.1% | 1.61 ± 0.0% | 1.71 ± 1.9% | 1.84 ± 0.4% |
| 65536 | by 7 | 1.87 ± 0.4% | 1.62 ± 0.1% | 1.72 ± 0.9% | 1.87 ± 0.6% |
| 1048576 | by 7 | 1.87 ± 0.1% | 1.58 ± 0.3% | 1.72 ± 1.2% | 1.83 ± 0.3% |

Each counterpart's speed relative to ours: whole: xxHash 0.79x,
xxhash-rust 1.01x, twox-hash 1.01x; by 7: xxHash 0.86x, xxhash-rust
0.93x, twox-hash 0.99x.

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
| xxh32 | x86_64-unknown-linux-gnu | 614 | 355 |
| xxh64 | aarch64-apple-darwin | 632 | 552 |
| xxh64 | x86_64-unknown-linux-gnu | 864 | 570 |

On aarch64-apple-darwin, measured 2026-11-07, at `86d2427`,
`compute-xxhash` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On x86_64-unknown-linux-gnu, measured 2026-11-10, at `037ffbb`,
`compute-xxhash` 0.0.4, by rustc 1.97.0 (2d8144b78 2026-07-07).

<!-- filer-bench: end -->
