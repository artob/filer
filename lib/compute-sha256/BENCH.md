# compute-sha256 benchmarks

The figures of `compute-sha256`'s SHA-256, alone and against the main
alternatives, measured by the protocol of this repository's
[`bench/METHOD.md`](../../bench/METHOD.md): the same inputs, machines,
and builds for every implementation, each figure a median with its 95%
confidence interval, and every result checked against the others'.

## Counterparts

| Implementation | Crate | Language |
| --- | --- | --- |
| sha2 | `sha2` | Rust, with SHA instructions |
| ring | `ring` | C and assembly |

## Inputs

Buffers of random bytes, of 16 B, 256 B, 4 KiB, 64 KiB, and 1 MiB, fed
whole, by one update, and in pieces of 7 bytes, for SHA-256.

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

#### SHA-256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.03 ± 0.5% | 0.16 ± 0.4% | 0.10 ± 0.9% |
| 256 | whole | 0.12 ± 0.4% | 0.81 ± 0.2% | 0.70 ± 0.2% |
| 4096 | whole | 0.16 ± 0.5% | 1.20 ± 0.2% | 1.18 ± 0.4% |
| 65536 | whole | 0.16 ± 0.5% | 1.23 ± 0.2% | 1.23 ± 0.1% |
| 1048576 | whole | 0.16 ± 0.3% | 1.23 ± 0.1% | 1.23 ± 0.2% |
| 16 | by 7 | 0.03 ± 0.5% | 0.14 ± 0.3% | 0.09 ± 2.5% |
| 256 | by 7 | 0.11 ± 0.4% | 0.46 ± 0.2% | 0.33 ± 0.9% |
| 4096 | by 7 | 0.13 ± 0.4% | 0.55 ± 0.2% | 0.39 ± 0.5% |
| 65536 | by 7 | 0.13 ± 0.2% | 0.56 ± 0.6% | 0.40 ± 0.6% |
| 1048576 | by 7 | 0.13 ± 0.7% | 0.56 ± 0.3% | 0.40 ± 0.7% |

Each counterpart's speed relative to ours: whole: sha2 6.73x, ring
5.92x; by 7: sha2 4.34x, ring 3.02x.

### Ours with `intrinsics`

#### SHA-256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.12 ± 0.9% | 0.16 ± 1.0% | 0.10 ± 0.6% |
| 256 | whole | 0.74 ± 0.8% | 0.82 ± 0.1% | 0.70 ± 0.8% |
| 4096 | whole | 1.18 ± 0.2% | 1.19 ± 0.2% | 1.17 ± 0.3% |
| 65536 | whole | 1.23 ± 0.2% | 1.23 ± 0.3% | 1.23 ± 0.2% |
| 1048576 | whole | 1.23 ± 0.2% | 1.23 ± 0.4% | 1.22 ± 0.3% |
| 16 | by 7 | 0.11 ± 0.6% | 0.14 ± 1.0% | 0.09 ± 0.3% |
| 256 | by 7 | 0.36 ± 1.0% | 0.46 ± 0.3% | 0.32 ± 0.5% |
| 4096 | by 7 | 0.45 ± 0.7% | 0.55 ± 0.5% | 0.39 ± 0.8% |
| 65536 | by 7 | 0.46 ± 0.4% | 0.56 ± 0.6% | 0.40 ± 0.7% |
| 1048576 | by 7 | 0.46 ± 0.8% | 0.57 ± 0.4% | 0.39 ± 0.9% |

Each counterpart's speed relative to ours: whole: sha2 1.09x, ring
0.96x; by 7: sha2 1.24x, ring 0.86x.

## Code size

The bytes of `.text` each algorithm adds to a minimal `#![no_std]`
program that computes a checksum or hash with it, built portable with
fat LTO; read-only data, such as tables, is not counted.

<!-- filer-bench: size -->
<!-- filer-bench: end -->
