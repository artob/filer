#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Compares `filer-ntfs` with the Linux kernel's `ntfs3` driver: for each
# image, the `ntfs-manifest` example's listing must equal what Linux shows with
# the image loop-mounted read-only: names, kinds, sizes, contents, and
# modification, creation, and access times. `ntfs3` has no named streams
# as attributes, so the flags and extended attributes are compared with
# what ntfs-3g shows instead: its `system.ntfs_attrib_be`, and its `user.`
# attributes. The images are the committed fixtures and fresh ones
# `tests/fixtures/make.sh` makes.
#
# Listings are compared sorted: `filer-ntfs` lists a directory in its
# index's collation order, as Windows does, while `ntfs3` lists the
# entries of an index node that separate its children after them.
#
# Some entries differ by design, and `tests/fixtures.rs` checks them
# instead: absolute links and junctions (`ntfs3` keeps the drive, as in
# `C:/target.txt`, or makes `./Sub`, where Filer resolves from the
# volume's root), WSL symlinks (plain files to `ntfs3`), and reparse
# points of other tags (`ntfs3` reads their `$DATA` as contents, where
# Filer refuses to), files compressed by WOF (which `ntfs3` decompresses,
# Filer refuses to, and ntfs-3g shows as symlinks), and encrypted files
# (whose streams Filer cannot list, as it refuses to open them). Interix special files (symlinks, FIFOs, sockets, and
# devices) are regular files to `ntfs3`, and are compared with ntfs-3g
# only. `ntfs3` also reports no birth time for symlinks.
#
# Each image is a job of its own, as many at once as there are cores (or
# `FILER_JOBS`), each one's report printed whole, in order.
#
# Usage: lib/filer-ntfs/tests/differential.sh, from the repository root,
# on Linux with ntfs-3g, python3, GNU coreutils, the `ntfs3` module, and
# `sudo` for mounting.

set -eu
export TZ=UTC LC_ALL=C.UTF-8
# shellcheck source=lib/filer-spec/tests/jobs.sh
. lib/filer-spec/tests/jobs.sh
# shellcheck source=lib/filer-ntfs/tests/linux.sh
. lib/filer-ntfs/tests/linux.sh

# Expands sparse fixture `$1` into image `$2`.
unsparse() {
    python3 -I - "$1" "$2" <<'PY'
import struct, sys
data = open(sys.argv[1], "rb").read()
assert data[:15] == b"FILER-SPARSE-2\n"
image = bytearray(struct.unpack("<Q", data[15:23])[0])
rest = data[23:]
while rest:
    first, count = struct.unpack("<II", rest[:8])
    sector = rest[8:520]
    for i in range(first, first + count):
        image[i * 512:(i + 1) * 512] = sector
    rest = rest[520:]
open(sys.argv[2], "wb").write(image)
PY
}

# Compares image `$3`, expanded from sparse fixture `$2`, with what
# `ntfs3` and ntfs-3g show of it: the one kind of job.
perform() {
    unsparse "$2" "$scratch/$3.img"
    check "$scratch/$3.img" "$3"
}

if [ "${1:-}" = --job ]; then
    job "$2"
fi

work=$(mktemp -d -p /var/tmp)
trap clean_work EXIT
failed=0

# A release build, as a debug one reads each volume some twenty times
# slower.
cargo build -q --release -p filer-ntfs --example ntfs-manifest
manifest="$PWD/target/release/examples/ntfs-manifest"

# The committed fixtures, and fresh ones, each a job.
mkdir "$work/fresh"
lib/filer-ntfs/tests/fixtures/make.sh "$work/fresh"
for sparse in lib/filer-ntfs/tests/fixtures/*.sparse "$work"/fresh/*.sparse; do
    case $sparse in "$work"/*) kind=fresh ;; *) kind=fixture ;; esac
    add image "$sparse" "$kind-$(basename "$sparse" .sparse)"
done
run
exit $failed
