#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Compares `filer-xfs` with the Linux kernel's `xfs` driver: for each
# image, the `xfs-manifest` example's listing must equal what Linux shows with
# the image loop-mounted read-only: names, kinds, sizes, contents, symlink
# targets, the modification, change, access, and creation times to the
# nanosecond, the inode flags Filer reports (immutable, append-only,
# compressed, encrypted, and verity), and the extended attributes of
# files and directories. The images are the committed fixtures and fresh
# ones `tests/fixtures/make.sh` makes; those of 64 KiB blocks, which x86
# Linux cannot mount, are skipped, and checked by `tests/fixtures.rs`
# instead.
#
# Then logs as the kernel leaves them, with each sector size and log
# stripe unit: clean, before and after the log wraps, which must be read
# alike; shut down mid-write, which must be read as the kernel recovers
# it; and recovered by the kernel.
#
# Listings are compared sorted, as the orders need not agree. Names that
# are not UTF-8 are shown with `\xHH` escapes, and such entries are
# compared by their metadata only: Filer's paths are UTF-8.
#
# Fresh fixtures keep the logs the kernel wrote, as Linux refuses to
# mount version 5 volumes whose logs were reformatted, as the committed
# ones are: those are skipped, as is the data device of a volume with a
# realtime device, which Linux mounts only with that device.
#
# Each image and each log set-up is a job of its own, with its own mount
# point and files; as many run at once as there are cores (or
# `FILER_JOBS`), and each one's report is printed whole, in order,
# when its phase ends, with how long the run has taken.
#
# Usage: lib/filer-xfs/tests/differential.sh [images] [logs], from the
# repository root, on Linux with xfsprogs, python3, and `sudo` for
# mounting; with no phase named, both run.

set -eu
export TZ=UTC LC_ALL=C.UTF-8
# shellcheck source=lib/filer-spec/tests/jobs.sh
. lib/filer-spec/tests/jobs.sh
# shellcheck source=lib/filer-xfs/tests/linux.sh
. lib/filer-xfs/tests/linux.sh

# Prints the byte offset of the log in image `$1`.
log_offset() {
    field() {
        xfs_db -r -c "sb 0" -c "print $2" "$1" | sed 's/.*= //'
    }
    start=$(field "$1" logstart)
    agblocks=$(field "$1" agblocks)
    agblklog=$(field "$1" agblklog)
    blocksize=$(field "$1" blocksize)
    ag=$((start >> agblklog))
    block=$((start & ((1 << agblklog) - 1)))
    echo $(((ag * agblocks + block) * blocksize))
}

# Changes the times of a few thousand files in directory `$1`, syncing
# after each round, until the log of image `$2`, at byte `$3`, has wrapped:
# each sync logs every inode changed since the last again, so a few dozen
# rounds fill a 64 MiB log.
wrap() {
    sudo python3 -I - "$1" "$2" "$3" <<'PY'
import os, struct, sys
mnt, img, at = sys.argv[1], sys.argv[2], int(sys.argv[3])
def cycle():
    with open(img, "rb") as f:
        f.seek(at)
        first, second = struct.unpack(">II", f.read(8))
    # A record header's sector starts with its magic, then the cycle.
    return second if first == 0xFEEDBABE else first
base = os.path.join(mnt, "wrap")
os.mkdir(base)
paths = [os.path.join(base, f"{i:05}") for i in range(8000)]
for path in paths:
    open(path, "wb").close()
os.sync()
for round in range(1, 400):
    if cycle() >= 2:
        sys.exit()
    for n, path in enumerate(paths):
        os.utime(path, ns=(round * 10**9, round * 10**9 + n))
    os.sync()
sys.exit("FAILED: the log did not wrap")
PY
}

# Changes what is in directory `$1`, in a directory `$2` of its own, then
# shuts its volume down, so that its log needs recovery: files written,
# renamed, and fragmented; directories grown to each form, and one
# removed, so that its blocks are cancelled; attributes set, one remote;
# and a symlink too long for the inode.
shut() {
    sudo python3 -I - "$1" "$2" <<'PY'
import os, shutil, sys
base = os.path.join(sys.argv[1], sys.argv[2])
os.mkdir(base)
def put(path, data):
    with open(os.path.join(base, path), "wb") as f:
        f.write(data)
for i in range(50):
    put(f"f{i}", str(i).encode() * i)
os.mkdir(os.path.join(base, "gone"))
for i in range(500):
    put(f"gone/g{i:04}", b"g")
os.sync()
shutil.rmtree(os.path.join(base, "gone"))
os.mkdir(os.path.join(base, "big"))
for i in range(2000):
    put(f"big/b{i:04}", b"")
put("attrs", b"attrs")
for i in range(30):
    os.setxattr(os.path.join(base, "attrs"), f"user.a{i:02}", b"v" * 40)
os.setxattr(os.path.join(base, "attrs"), "user.remote", bytes(range(256)) * 20)
with open(os.path.join(base, "frag"), "wb") as f:
    for i in range(0, 400, 2):
        f.seek(i * 4096)
        f.write(bytes([i % 251]) * 4096)
os.symlink("x/" * 450, os.path.join(base, "link"))
os.rename(os.path.join(base, "f0"), os.path.join(base, "renamed"))
os.sync()
PY
    # A shutdown reaches the volume holding `$1`: never let that be the
    # root's.
    mountpoint -q "$1" || exit 1
    sudo xfs_io -x -c shutdown "$1"
}

# Checks the log of a volume formatted with options `$2`, called `$1`.
logs() {
    name=$1
    img="$scratch/log.img"
    truncate -s 320M "$img"
    # shellcheck disable=SC2086
    mkfs.xfs -q -f $2 "$img" 2>/dev/null
    at=$(log_offset "$img")
    sudo mount -o loop "$img" "$mnt"
    sudo sh -c "echo first > '$mnt/first'"
    sudo umount "$mnt"
    compare "$img" "$name, clean"
    sudo mount -o loop "$img" "$mnt"
    shut "$mnt" changes
    sudo umount "$mnt"
    recovered "$img" "$name, shut down"
    sudo mount -o loop "$img" "$mnt"
    sudo umount "$mnt"
    compare "$img" "$name, recovered"
    sudo mount -o loop "$img" "$mnt"
    wrap "$mnt" "$img" "$at"
    sudo umount "$mnt"
    compare "$img" "$name, wrapped"
    sudo mount -o loop "$img" "$mnt"
    shut "$mnt" more
    sudo umount "$mnt"
    recovered "$img" "$name, wrapped, shut down"
    sudo mount -o loop "$img" "$mnt"
    sudo umount "$mnt"
    compare "$img" "$name, wrapped, recovered"
}

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

# Performs a job of kind `$1`: comparing image `$3` with Linux, expanded
# from sparse fixture `$2`; or checking the logs of set-up `$2`, of mkfs
# options `$3`.
perform() {
    case $1 in
        image)
            img="$scratch/$3.img"
            unsparse "$2" "$img"
            compare "$img" "$3"
            ;;
        logs) logs "$2" "$3" ;;
    esac
}

if [ "${1:-}" = --job ]; then
    job "$2"
fi

phases=${*:-images logs}
work=$(mktemp -d -p /var/tmp)
trap clean_work EXIT
started=$(date +%s)
failed=0

# Prints how long the run has taken, when phase `$1` ends.
took() {
    elapsed=$(($(date +%s) - started))
    echo "-- $1 after $((elapsed / 60)) min $((elapsed % 60)) s"
}

# A release build, as a debug one reads each volume some seventy times
# slower.
cargo build -q --release -p filer-xfs --example xfs-manifest
manifest="$PWD/target/release/examples/xfs-manifest"

case " $phases " in *" images "*)
    # The committed fixtures, and fresh ones.
    mkdir "$work/fresh"
    KEEP_LOG=1 lib/filer-xfs/tests/fixtures/make.sh "$work/fresh"
    took "making fresh fixtures"
    for sparse in lib/filer-xfs/tests/fixtures/*.sparse "$work"/fresh/*.sparse; do
        case $sparse in "$work"/*) kind=fresh ;; *) kind=fixture ;; esac
        add image "$sparse" "$kind-$(basename "$sparse" .sparse)"
    done
    run
    took "comparing images"
    ;;
esac

case " $phases " in *" logs "*)
    v5="-m crc=1,finobt=0,rmapbt=0,reflink=0,bigtime=0,inobtcount=0"
    v5="$v5 -i sparse=0,nrext64=0"
    add logs "log" "-m crc=0"
    add logs "log of 4 KiB sectors" "-m crc=0 -s size=4096"
    add logs "log of 32 KiB stripes" "-m crc=0 -l su=32k"
    add logs "log of 256 KiB stripes" "-m crc=0 -l su=256k"
    add logs "version 5 log" "$v5"
    add logs "version 5 log of 4 KiB sectors" "$v5 -s size=4096"
    add logs "version 5 log of 256 KiB stripes" "$v5 -l su=256k"
    add logs "default log" ""
    add logs "log with the newest features" "-n parent=1 -m metadir=1"
    run
    took "checking logs"
    ;;
esac

exit $failed
