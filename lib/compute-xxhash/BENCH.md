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

64-bit x86 on a current AMD core and a current Intel core, both bare
metal, and on a cloud VM; and 64-bit ARM on Apple M4 and on a Neoverse
server.

Measured so far: AMD EPYC 7551P (Zen 1), 4 vCPUs under KVM, Linux 7.0,
Rust 1.97.0, 2026-11-06.

## Throughput

GB/s, each the median of five or more processes with its 95%
confidence interval: a row per size and way of feeding, a column per
implementation; then each counterpart's speed relative to ours, the
geometric mean over sizes. The counterparts pick the CPU's
instructions at run time.

### Ours portable, as by default

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

Each counterpart's speed relative to ours: whole: xxHash 0.53x, xxhash-
rust 0.93x, twox-hash 0.96x; by 7: xxHash 0.84x, xxhash-rust 0.95x,
twox-hash 0.96x.

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

Each counterpart's speed relative to ours: whole: xxHash 0.95x, xxhash-
rust 0.99x, twox-hash 1.03x; by 7: xxHash 0.99x, xxhash-rust 0.93x,
twox-hash 0.96x.

## Code size

The bytes of `.text` it adds to a minimal `#![no_std]` binary: not
measured yet.
