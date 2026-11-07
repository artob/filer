# compute-sha512 benchmarks

The figures of `compute-sha512`'s SHA-512 and SHA-512/256, alone and
against the main alternatives, measured by the protocol of this
repository's [`bench/METHOD.md`](../../bench/METHOD.md): the same
inputs, machines, and builds for every implementation, each figure a
median with its 95% confidence interval, and every result checked
against the others'.

## Counterparts

| Implementation | Crate | Language |
| --- | --- | --- |
| sha2 | `sha2` | Rust, with SHA instructions |
| ring | `ring` | C and assembly |

## Inputs

Buffers of random bytes, of 16 B, 256 B, 4 KiB, 64 KiB, and 1 MiB, fed
whole, by one update, and in pieces of 7 bytes, for SHA-512 and
SHA-512/256.

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

#### SHA-512

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.02 ± 0.3% | 0.03 ± 0.5% | 0.03 ± 0.8% |
| 256 | whole | 0.15 ± 0.4% | 0.17 ± 0.4% | 0.22 ± 0.8% |
| 4096 | whole | 0.24 ± 0.4% | 0.26 ± 0.4% | 0.34 ± 0.6% |
| 65536 | whole | 0.24 ± 0.7% | 0.27 ± 0.5% | 0.36 ± 0.8% |
| 1048576 | whole | 0.24 ± 0.4% | 0.27 ± 0.5% | 0.35 ± 0.4% |
| 16 | by 7 | 0.02 ± 0.7% | 0.03 ± 0.7% | 0.03 ± 0.7% |
| 256 | by 7 | 0.13 ± 0.5% | 0.15 ± 0.2% | 0.16 ± 0.7% |
| 4096 | by 7 | 0.18 ± 1.0% | 0.20 ± 0.7% | 0.22 ± 0.8% |
| 65536 | by 7 | 0.19 ± 0.5% | 0.21 ± 0.4% | 0.23 ± 1.0% |
| 1048576 | by 7 | 0.19 ± 0.3% | 0.21 ± 0.3% | 0.23 ± 0.6% |

Each counterpart's speed relative to ours: whole: sha2 1.14x, ring
1.43x; by 7: sha2 1.14x, ring 1.25x.

#### SHA-512/256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.03 ± 0.4% | 0.03 ± 0.3% | 0.03 ± 0.8% |
| 256 | whole | 0.15 ± 0.4% | 0.17 ± 0.6% | 0.22 ± 0.6% |
| 4096 | whole | 0.24 ± 0.8% | 0.26 ± 0.2% | 0.34 ± 0.8% |
| 65536 | whole | 0.24 ± 0.3% | 0.27 ± 0.4% | 0.35 ± 0.4% |
| 1048576 | whole | 0.24 ± 0.9% | 0.27 ± 0.9% | 0.35 ± 0.6% |
| 16 | by 7 | 0.02 ± 1.0% | 0.03 ± 0.4% | 0.03 ± 0.4% |
| 256 | by 7 | 0.13 ± 0.6% | 0.15 ± 0.3% | 0.16 ± 0.5% |
| 4096 | by 7 | 0.18 ± 0.2% | 0.21 ± 0.4% | 0.21 ± 0.3% |
| 65536 | by 7 | 0.19 ± 0.3% | 0.21 ± 0.1% | 0.21 ± 0.2% |
| 1048576 | by 7 | 0.19 ± 0.6% | 0.21 ± 0.5% | 0.21 ± 0.4% |

Each counterpart's speed relative to ours: whole: sha2 1.14x, ring
1.43x; by 7: sha2 1.16x, ring 1.21x.

### Ours with `intrinsics`

#### SHA-512

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.03 ± 0.7% | 0.03 ± 1.0% | 0.03 ± 0.4% |
| 256 | whole | 0.15 ± 0.4% | 0.17 ± 0.9% | 0.21 ± 0.9% |
| 4096 | whole | 0.23 ± 1.0% | 0.26 ± 0.5% | 0.34 ± 0.7% |
| 65536 | whole | 0.24 ± 0.7% | 0.27 ± 0.2% | 0.35 ± 0.4% |
| 1048576 | whole | 0.24 ± 0.7% | 0.27 ± 0.3% | 0.36 ± 0.4% |
| 16 | by 7 | 0.02 ± 0.3% | 0.03 ± 0.5% | 0.03 ± 0.9% |
| 256 | by 7 | 0.12 ± 0.8% | 0.15 ± 0.6% | 0.16 ± 0.7% |
| 4096 | by 7 | 0.18 ± 0.4% | 0.21 ± 0.6% | 0.22 ± 0.6% |
| 65536 | by 7 | 0.18 ± 0.3% | 0.22 ± 0.8% | 0.23 ± 0.4% |
| 1048576 | by 7 | 0.18 ± 0.4% | 0.22 ± 0.7% | 0.23 ± 0.8% |

Each counterpart's speed relative to ours: whole: sha2 1.14x, ring
1.42x; by 7: sha2 1.20x, ring 1.27x.

#### SHA-512/256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.02 ± 0.5% | 0.03 ± 0.9% | 0.03 ± 0.7% |
| 256 | whole | 0.15 ± 0.8% | 0.17 ± 0.5% | 0.21 ± 0.6% |
| 4096 | whole | 0.23 ± 0.9% | 0.26 ± 0.5% | 0.34 ± 0.7% |
| 65536 | whole | 0.24 ± 0.3% | 0.27 ± 0.2% | 0.36 ± 0.6% |
| 1048576 | whole | 0.24 ± 1.0% | 0.27 ± 0.7% | 0.35 ± 0.8% |
| 16 | by 7 | 0.02 ± 0.9% | 0.03 ± 0.5% | 0.03 ± 1.0% |
| 256 | by 7 | 0.13 ± 0.5% | 0.16 ± 0.9% | 0.16 ± 0.6% |
| 4096 | by 7 | 0.18 ± 0.3% | 0.22 ± 0.4% | 0.21 ± 0.4% |
| 65536 | by 7 | 0.18 ± 0.2% | 0.22 ± 0.4% | 0.22 ± 0.3% |
| 1048576 | by 7 | 0.19 ± 0.8% | 0.22 ± 0.5% | 0.22 ± 0.9% |

Each counterpart's speed relative to ours: whole: sha2 1.15x, ring
1.43x; by 7: sha2 1.22x, ring 1.22x.

## Code size

The bytes of `.text` each algorithm adds to a minimal `#![no_std]`
program that computes a checksum or hash with it, built portable with
fat LTO; read-only data, such as tables, is not counted.

<!-- filer-bench: size -->
<!-- filer-bench: end -->
