# compute-fletcher benchmarks

The figures of `compute-fletcher`'s Fletcher checksums, alone and
against the main alternatives, measured by the protocol of this
repository's [`bench/METHOD.md`](../../bench/METHOD.md): the same
inputs, machines, and builds for every implementation, each figure a
median with its 95% confidence interval, and every result checked
against the others'.

## Counterparts

None: no other crate computes these as ZFS and APFS do, so that its
results could be checked against ours; they are measured alone.

## Inputs

Buffers of random bytes, of 16 B, 256 B, 4 KiB, 64 KiB, and 1 MiB, fed
whole, by one update, and in pieces of 7 bytes, for Fletcher-2,
Fletcher-4, and Fletcher-64.

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

#### Fletcher-2

| Bytes | Fed | ours |
| ---: | --- | ---: |
| 16 | whole | 1.79 ± 1.1% |
| 256 | whole | 24.91 ± 0.7% |
| 4096 | whole | 37.59 ± 0.2% |
| 65536 | whole | 34.91 ± 0.9% |
| 1048576 | whole | 35.01 ± 0.1% |
| 16 | by 7 | 0.77 ± 1.1% |
| 256 | by 7 | 1.57 ± 0.2% |
| 4096 | by 7 | 1.68 ± 0.0% |
| 65536 | by 7 | 1.69 ± 0.0% |
| 1048576 | by 7 | 1.68 ± 0.1% |

#### Fletcher-4

| Bytes | Fed | ours |
| ---: | --- | ---: |
| 16 | whole | 0.76 ± 1.9% |
| 256 | whole | 6.61 ± 1.2% |
| 4096 | whole | 11.63 ± 1.0% |
| 65536 | whole | 12.60 ± 0.2% |
| 1048576 | whole | 12.61 ± 0.2% |
| 16 | by 7 | 0.51 ± 2.2% |
| 256 | by 7 | 0.93 ± 0.4% |
| 4096 | by 7 | 1.01 ± 0.1% |
| 65536 | by 7 | 1.02 ± 0.0% |
| 1048576 | by 7 | 1.01 ± 0.1% |

#### Fletcher-64

| Bytes | Fed | ours |
| ---: | --- | ---: |
| 16 | whole | 1.14 ± 0.7% |
| 256 | whole | 9.31 ± 0.1% |
| 4096 | whole | 16.85 ± 0.1% |
| 65536 | whole | 17.87 ± 0.1% |
| 1048576 | whole | 17.91 ± 0.1% |
| 16 | by 7 | 0.58 ± 0.6% |
| 256 | by 7 | 1.08 ± 0.0% |
| 4096 | by 7 | 1.12 ± 0.1% |
| 65536 | by 7 | 1.13 ± 0.1% |
| 1048576 | by 7 | 1.13 ± 0.1% |

### Granite Rapids, ours portable, as by default

#### Fletcher-2

| Bytes | Fed | ours |
| ---: | --- | ---: |
| 16 | whole | 1.25 ± 1.8% |
| 256 | whole | 18.96 ± 1.0% |
| 4096 | whole | 50.50 ± 0.1% |
| 65536 | whole | 54.68 ± 0.1% |
| 1048576 | whole | 55.09 ± 0.1% |
| 16 | by 7 | 0.55 ± 0.9% |
| 256 | by 7 | 1.08 ± 0.3% |
| 4096 | by 7 | 1.27 ± 0.1% |
| 65536 | by 7 | 1.28 ± 0.0% |
| 1048576 | by 7 | 1.28 ± 0.1% |

#### Fletcher-4

| Bytes | Fed | ours |
| ---: | --- | ---: |
| 16 | whole | 0.66 ± 0.7% |
| 256 | whole | 5.37 ± 0.8% |
| 4096 | whole | 7.84 ± 0.5% |
| 65536 | whole | 8.24 ± 0.4% |
| 1048576 | whole | 8.24 ± 0.2% |
| 16 | by 7 | 0.41 ± 0.8% |
| 256 | by 7 | 0.72 ± 0.2% |
| 4096 | by 7 | 0.79 ± 0.1% |
| 65536 | by 7 | 0.79 ± 0.1% |
| 1048576 | by 7 | 0.79 ± 0.1% |

#### Fletcher-64

| Bytes | Fed | ours |
| ---: | --- | ---: |
| 16 | whole | 0.84 ± 2.7% |
| 256 | whole | 7.34 ± 1.2% |
| 4096 | whole | 13.83 ± 0.2% |
| 65536 | whole | 14.56 ± 0.1% |
| 1048576 | whole | 14.58 ± 0.2% |
| 16 | by 7 | 0.43 ± 1.5% |
| 256 | by 7 | 0.79 ± 0.2% |
| 4096 | by 7 | 0.86 ± 0.2% |
| 65536 | by 7 | 0.87 ± 0.2% |
| 1048576 | by 7 | 0.87 ± 0.1% |

### Zen 1, ours portable, as by default

#### Fletcher-2

| Bytes | Fed | ours |
| ---: | --- | ---: |
| 16 | whole | 0.43 ± 0.8% |
| 256 | whole | 6.09 ± 0.8% |
| 4096 | whole | 27.25 ± 0.5% |
| 65536 | whole | 36.76 ± 0.5% |
| 1048576 | whole | 35.53 ± 1.0% |
| 16 | by 7 | 0.23 ± 0.7% |
| 256 | by 7 | 0.46 ± 0.5% |
| 4096 | by 7 | 0.51 ± 1.0% |
| 65536 | by 7 | 0.50 ± 0.5% |
| 1048576 | by 7 | 0.50 ± 0.4% |

#### Fletcher-4

| Bytes | Fed | ours |
| ---: | --- | ---: |
| 16 | whole | 0.24 ± 0.4% |
| 256 | whole | 1.95 ± 0.9% |
| 4096 | whole | 3.49 ± 0.4% |
| 65536 | whole | 3.70 ± 0.6% |
| 1048576 | whole | 3.72 ± 0.6% |
| 16 | by 7 | 0.18 ± 1.5% |
| 256 | by 7 | 0.32 ± 0.8% |
| 4096 | by 7 | 0.33 ± 0.9% |
| 65536 | by 7 | 0.34 ± 0.5% |
| 1048576 | by 7 | 0.34 ± 0.3% |

#### Fletcher-64

| Bytes | Fed | ours |
| ---: | --- | ---: |
| 16 | whole | 0.36 ± 0.6% |
| 256 | whole | 3.75 ± 0.5% |
| 4096 | whole | 8.02 ± 0.7% |
| 65536 | whole | 8.69 ± 0.9% |
| 1048576 | whole | 8.77 ± 0.8% |
| 16 | by 7 | 0.20 ± 0.9% |
| 256 | by 7 | 0.36 ± 0.4% |
| 4096 | by 7 | 0.37 ± 0.9% |
| 65536 | by 7 | 0.37 ± 0.4% |
| 1048576 | by 7 | 0.38 ± 0.5% |

## Code size

The bytes of `.text` each algorithm adds to a minimal `#![no_std]`
program that computes a checksum or hash with it, built portable with
fat LTO; read-only data, such as tables, is not counted.

<!-- filer-bench: size -->

| Algorithm | Target | `opt-level` 3 | `opt-level` "s" |
| --- | --- | ---: | ---: |
| fletcher2 | aarch64-apple-darwin | 56 | 96 |
| fletcher2 | x86_64-unknown-linux-gnu | 198 | 91 |
| fletcher4 | aarch64-apple-darwin | 624 | 624 |
| fletcher4 | x86_64-unknown-linux-gnu | 970 | 764 |
| fletcher64 | aarch64-apple-darwin | 268 | 264 |
| fletcher64 | x86_64-unknown-linux-gnu | 476 | 328 |

On aarch64-apple-darwin, measured 2026-11-07, at `86d2427`,
`compute-fletcher` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On x86_64-unknown-linux-gnu, measured 2026-11-10, at `037ffbb`,
`compute-fletcher` 0.0.4, by rustc 1.97.0 (2d8144b78 2026-07-07).

<!-- filer-bench: end -->
