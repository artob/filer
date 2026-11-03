#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Makes the cpio fixtures with real tools: GNU cpio writes one tree, with
# a directory, an empty one, a symlink, a FIFO, and a file with two
# hardlinks, in each of its formats (`gnu-newc.cpio`, `gnu-crc.cpio`,
# `gnu-odc.cpio`, `gnu-bin.cpio`), and `initramfs.img` is an early archive
# of fake microcode followed by `gnu-newc.cpio`, as Linux distributions
# build their images, but uncompressed. `bsd-newc.cpio` and
# `bsd-odc.cpio` come from macOS's `bsdcpio`, from the same tree:
#
#     find . | LC_ALL=C sort | cpio -o -H newc > bsd-newc.cpio
#
# Usage: tests/data/make.sh [output-dir], on Linux with GNU cpio.

set -eu
out=$(cd "${1:-$(dirname "$0")}" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/tree/dir" "$work/tree/empty" \
    "$work/early/kernel/x86/microcode"
cd "$work/tree"
printf 'Hello, world!\n' > dir/hello.txt
printf 'shared\n' > hard1
ln hard1 dir/hard2
ln -s dir/hello.txt link
mkfifo fifo
find . -exec touch -h -d @1700000000 {} +
for format in newc crc odc bin; do
    find . | LC_ALL=C sort |
        cpio --quiet -o -H "$format" --reproducible > "$out/gnu-$format.cpio"
done
cd "$work/early"
printf 'not really microcode\n' > kernel/x86/microcode/GenuineIntel.bin
find . -exec touch -h -d @1700000000 {} +
find . | LC_ALL=C sort | cpio --quiet -o -H newc --reproducible > ../early.cpio
cat ../early.cpio "$out/gnu-newc.cpio" > "$out/initramfs.img"
