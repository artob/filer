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
# alike; shut down mid-write, which must be refused as needing recovery;
# and recovered by the kernel.
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

cargo build -q -p filer-xfs --example manifest
manifest="$PWD/target/debug/examples/manifest"

# The committed fixtures, and fresh ones.
mkdir "$work/fresh"
KEEP_LOG=1 lib/filer-xfs/tests/fixtures/make.sh "$work/fresh"
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

# Prints what the Linux kernel shows, in the manifest's format.
kernel_manifest() {
    # As root, as the fixtures' files are root's.
    sudo python3 -I - "$1" <<'PY'
import ctypes, fcntl, os, re, stat, struct, sys
root = os.fsencode(sys.argv[1])
libc = ctypes.CDLL(None, use_errno=True)
AT_FDCWD, AT_SYMLINK_NOFOLLOW, STATX_ALL = -100, 0x100, 0xFFF
STATX_BTIME = 0x800

def birth(path):
    buf = ctypes.create_string_buffer(256)
    if libc.statx(AT_FDCWD, path, AT_SYMLINK_NOFOLLOW, STATX_ALL, buf) != 0:
        raise OSError(ctypes.get_errno(), "statx")
    mask, = struct.unpack_from("<I", buf.raw, 0)
    if not mask & STATX_BTIME:
        return "-"
    secs, nanos = struct.unpack_from("<qI", buf.raw, 0x50)
    return f"{secs}.{nanos:09}"

def fnv1a(data):
    # A zero byte multiplies the hash by the prime, so runs of zeros are
    # hashed at once.
    h, prime, mask = 0xCBF29CE484222325, 0x100000001B3, 0xFFFFFFFFFFFFFFFF
    for run in re.finditer(rb"\x00+|[^\x00]+", data):
        chunk = run.group()
        if chunk[0] == 0:
            h = h * pow(prime, len(chunk), 1 << 64) & mask
        else:
            for b in chunk:
                h = (h ^ b) * prime & mask
    return h

def shown(name):
    # As Filer displays names: bytes that are not UTF-8 as `\xHH`.
    text = name.decode("utf-8", "surrogateescape")
    return "".join(f"\\x{ord(c) - 0xDC00:02X}" if 0xDC80 <= ord(c) <= 0xDCFF
                   else c for c in text)

# The inode flags Filer reports, by `FS_IOC_GETFLAGS` bit.
FLAGS = [(0x4, "compressed"), (0x10, "immutable"), (0x20, "append_only"),
         (0x800, "encrypted"), (0x100000, "verity")]
FS_IOC_GETFLAGS = 0x80086601

def joined(items):
    return ",".join(sorted(items)) or "-"

def flags(path):
    fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK)
    try:
        bits, = struct.unpack_from("<I", fcntl.ioctl(fd, FS_IOC_GETFLAGS, bytes(8)))
    finally:
        os.close(fd)
    return joined(name for bit, name in FLAGS if bits & bit)

def xattrs(path):
    return joined(f"{shown(name.encode('utf-8', 'surrogateescape'))}="
                  f"{fnv1a(os.getxattr(path, name)):016x}"
                  for name in os.listxattr(path))

def time(ns):
    return f"{ns // 10**9}.{ns % 10**9:09}"

KINDS = [(stat.S_ISDIR, "d"), (stat.S_ISLNK, "l"), (stat.S_ISFIFO, "p"),
         (stat.S_ISSOCK, "s"), (stat.S_ISCHR, "c"), (stat.S_ISBLK, "b")]

def walk(rel, rel_shown):
    # `listdir` returns entries in the order the directory holds them.
    for name in os.listdir(os.path.join(root, rel)):
        path = os.path.join(rel, name)
        full = os.path.join(root, path)
        path_shown = shown(name) if not rel else f"{rel_shown}/{shown(name)}"
        st = os.lstat(full)
        kind = next((k for test, k in KINDS if test(st.st_mode)), "f")
        times = "\t".join([time(st.st_mtime_ns), time(st.st_ctime_ns),
                           time(st.st_atime_ns), birth(full)])
        foreign = path_shown != path.decode("utf-8", "replace")
        if foreign or kind in "pscb":
            what = "-"
        elif kind == "l":
            what = os.readlink(full).decode("utf-8", "replace")
        elif kind == "d":
            what = "-"
        else:
            what = f"{fnv1a(open(full, 'rb').read()):016x}"
        attributes = "-" if foreign or kind not in "fd" else xattrs(full)
        bits = flags(full) if kind in "fd" else "-"
        print(f"{path_shown}\t{kind}\t{st.st_size}\t{times}\t{what}\t{bits}\t{attributes}")
        if kind == "d" and not foreign:
            walk(path, path_shown)

walk(b"", "")
PY
}

failed=0

# Compares the manifest of image `$1`, called `$2`, with what the kernel
# shows.
compare() {
    if ! sudo mount -o loop,ro "$1" "$mnt" 2>/dev/null; then
        echo "skipped: $2 (Linux cannot mount it here)"
        return
    fi
    kernel_manifest "$mnt" | sort > "$work/kernel"
    sudo umount "$mnt"
    if ! "$manifest" "$1" > "$work/filer" 2> "$work/error"; then
        echo "FAILED: $2: $(cat "$work/error")"
        failed=1
        return
    fi
    sort -o "$work/filer" "$work/filer"
    if diff -u "$work/kernel" "$work/filer" > "$work/diff"; then
        echo "same: $2 ($(wc -l < "$work/filer") entries)"
    else
        echo "DIFFERENT: $2"
        head -40 "$work/diff"
        failed=1
    fi
}

for img in "$work"/*.img; do
    compare "$img" "$(basename "$img" .img)"
done

# Checks that the manifest of image `$1`, called `$2`, is refused as
# needing recovery.
refused() {
    if "$manifest" "$1" > /dev/null 2> "$work/error"; then
        echo "NOT REFUSED: $2"
        failed=1
    elif grep -q "log needs recovery" "$work/error"; then
        echo "refused: $2"
    else
        echo "FAILED: $2: $(cat "$work/error")"
        failed=1
    fi
}

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

# Writes files into directory `$1`, then shuts its volume down, so that
# its log needs recovery.
shut() {
    sudo sh -c "for i in \$(seq 50); do echo \$i > '$1/s\$i'; done; sync"
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
    shut "$mnt"
    sudo umount "$mnt"
    refused "$img" "$name, shut down"
    sudo mount -o loop "$img" "$mnt"
    sudo umount "$mnt"
    compare "$img" "$name, recovered"
    sudo mount -o loop "$img" "$mnt"
    wrap "$mnt" "$img" "$at"
    sudo umount "$mnt"
    compare "$img" "$name, wrapped"
    sudo mount -o loop "$img" "$mnt"
    shut "$mnt"
    sudo umount "$mnt"
    refused "$img" "$name, wrapped, shut down"
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
exit $failed
