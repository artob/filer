#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Compares `filer-ext4` with the Linux kernel's `ext4` driver, which also
# mounts ext2 and ext3: for each image, the `ext4-manifest` example's listing
# must equal what Linux shows with the image mounted read-only, its
# journal recovered if need be: names, kinds, sizes, contents, symlink
# targets, the modification, change, access, and creation times to the
# nanosecond, the inode flags Filer reports (immutable, append-only,
# compressed, encrypted, and verity), and the extended attributes of
# files and directories. The images are the committed fixtures and fresh
# ones `tests/fixtures/make.sh` makes; those of 64 KiB blocks, which x86 Linux
# cannot mount, are skipped, and checked by `tests/fixtures.rs` instead.
#
# Listings are compared sorted: Linux lists a hashed (htree) directory in
# hash order, where `filer-ext4` lists every directory in the order its
# blocks hold the entries. Names that are not UTF-8 are shown with `\xHH`
# escapes, and such entries are compared by their metadata only: Filer's
# paths are UTF-8.
#
# Each image is a job of its own, as many at once as there are cores (or
# `FILER_JOBS`), each one's report printed whole, in order.
#
# Usage: lib/filer-ext4/tests/differential.sh, from the repository root,
# on Linux with e2fsprogs, python3, and `sudo` for mounting.

set -eu
export TZ=UTC LC_ALL=C.UTF-8
# shellcheck source=lib/filer-spec/tests/jobs.sh
. lib/filer-spec/tests/jobs.sh

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

# Prints what the Linux kernel shows, in the manifest's format.
kernel_manifest() {
    # As root, as `lost+found` is root's alone.
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

# Compares image `$3`, expanded from sparse fixture `$2`, with what Linux
# shows of it: the one kind of job. A volume whose journal needs recovery
# is read as replaying the journal leaves it, which Linux writes to the
# image, so Filer reads it first. As it is on disk, such a volume is
# inconsistent, which Linux reports as corruption: `tests/fixtures.rs`
# checks that reading instead.
perform() {
    img="$scratch/$3.img"
    unsparse "$2" "$img"
    "$manifest" "$img" | sort > "$scratch/filer"
    # Writable, unlike `mount -o loop,ro`'s, as recovery writes to it.
    dev=$(sudo losetup --find --show "$img")
    if ! sudo mount -o ro "$dev" "$mnt" 2>/dev/null; then
        sudo losetup -d "$dev"
        echo "skipped: $3 (Linux cannot mount it here)"
        return
    fi
    kernel_manifest "$mnt" | sort > "$scratch/kernel"
    sudo umount "$mnt"
    sudo losetup -d "$dev"
    if diff -u "$scratch/kernel" "$scratch/filer" > "$scratch/diff"; then
        echo "same: $3 ($(wc -l < "$scratch/filer") entries)"
    else
        echo "DIFFERENT: $3"
        head -40 "$scratch/diff"
        failed=1
    fi
}

if [ "${1:-}" = --job ]; then
    job "$2"
fi

work=$(mktemp -d -p /var/tmp)
trap clean_work EXIT
failed=0

# A release build, as a debug one reads each volume some twenty times
# slower.
cargo build -q --release -p filer-ext4 --example ext4-manifest
manifest="$PWD/target/release/examples/ext4-manifest"

# The committed fixtures, and fresh ones, each a job.
mkdir "$work/fresh"
lib/filer-ext4/tests/fixtures/make.sh "$work/fresh"
for sparse in lib/filer-ext4/tests/fixtures/*.sparse "$work"/fresh/*.sparse; do
    case $sparse in "$work"/*) kind=fresh ;; *) kind=fixture ;; esac
    add image "$sparse" "$kind-$(basename "$sparse" .sparse)"
done
run
exit $failed
