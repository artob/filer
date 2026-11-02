#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Compares `filer-xfs` with the Linux kernel's `xfs` driver: for each
# image, the `manifest` example's listing must equal what Linux shows with
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
# It takes about 48 minutes on a host of four cores, printing how long at
# the end of each phase: under a minute making the fresh fixtures, 12
# comparing images, and 36 checking logs, each of nine set-ups twice
# wrapping a 64 MiB log. TODO.md (XFS-17) lists ways to make it faster.
#
# Usage: lib/filer-xfs/tests/differential.sh, from the repository root,
# on Linux with xfsprogs, python3, and `sudo` for mounting.

set -eu
export TZ=UTC LC_ALL=C.UTF-8
work=$(mktemp -d -p /var/tmp)
mnt="$work/mnt"
mkdir "$mnt"
cleanup() {
    if mountpoint -q "$mnt"; then sudo umount "$mnt"; fi
    sudo rm -rf "$work"
}
trap cleanup EXIT
started=$(date +%s)

# Prints how long the run has taken, when phase `$1` ends.
took() {
    echo "-- $1 after $((($(date +%s) - started) / 60)) min"
}

cargo build -q -p filer-xfs --example manifest
manifest="$PWD/target/debug/examples/manifest"

# The committed fixtures, and fresh ones.
mkdir "$work/fresh"
KEEP_LOG=1 lib/filer-xfs/tests/fixtures/make.sh "$work/fresh"
took "making fresh fixtures"
for sparse in lib/filer-xfs/tests/fixtures/*.sparse "$work"/fresh/*.sparse; do
    case $sparse in "$work"/*) kind=fresh ;; *) kind=fixture ;; esac
    python3 -I - "$sparse" "$work/$kind-$(basename "$sparse" .sparse).img" <<'PY'
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
done

# shellcheck source=lib/filer-xfs/tests/linux.sh
. lib/filer-xfs/tests/linux.sh
failed=0

for img in "$work"/*.img; do
    compare "$img" "$(basename "$img" .img)"
done
took "comparing images"

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

# Prints the cycle of the log's first sector, at byte `$2` of image `$1`.
cycle() {
    python3 -I - "$1" "$2" <<'PY'
import struct, sys
with open(sys.argv[1], "rb") as f:
    f.seek(int(sys.argv[2]))
    first, second = struct.unpack(">II", f.read(8))
# A record header's sector starts with its magic, then the cycle.
print(second if first == 0xFEEDBABE else first)
PY
}

# Creates and deletes files in directory `$1` until the log of image `$2`,
# at byte `$3`, has wrapped.
wrap() {
    for _ in $(seq 400); do
        sudo python3 -I - "$1" <<'PY'
import os, sys
for i in range(1000):
    open(os.path.join(sys.argv[1], f"{i:04}" + "w" * 250), "wb").close()
os.sync()
for i in range(1000):
    os.remove(os.path.join(sys.argv[1], f"{i:04}" + "w" * 250))
os.sync()
PY
        if [ "$(cycle "$2" "$3")" -ge 2 ]; then
            return
        fi
    done
    echo "FAILED: the log did not wrap"
    exit 1
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
    img="$work/log.img"
    rm -f "$img"
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

logs "log" "-m crc=0"
logs "log of 4 KiB sectors" "-m crc=0 -s size=4096"
logs "log of 32 KiB stripes" "-m crc=0 -l su=32k"
logs "log of 256 KiB stripes" "-m crc=0 -l su=256k"
v5="-m crc=1,finobt=0,rmapbt=0,reflink=0,bigtime=0,inobtcount=0"
v5="$v5 -i sparse=0,nrext64=0"
logs "version 5 log" "$v5"
logs "version 5 log of 4 KiB sectors" "$v5 -s size=4096"
logs "version 5 log of 256 KiB stripes" "$v5 -l su=256k"
logs "default log" ""
logs "log with the newest features" "-n parent=1 -m metadir=1"
took "checking logs"
exit $failed
