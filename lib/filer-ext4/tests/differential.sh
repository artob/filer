#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Compares `filer-ext4` with the Linux kernel's `ext4` driver, which also
# mounts ext2 and ext3: for each image, the `manifest` example's listing
# must equal what Linux shows with the image loop-mounted read-only:
# names in directory order, kinds, sizes, contents, symlink targets, and
# the modification, change, access, and creation times, to the
# nanosecond. The images are the committed fixtures and fresh ones
# `tests/fixtures/make.sh` makes; those of 64 KiB blocks, which x86 Linux
# cannot mount, are skipped, and checked by `tests/fixtures.rs` instead.
#
# Listings are compared sorted: Linux lists a hashed (htree) directory in
# hash order, where `filer-ext4` lists every directory in the order its
# blocks hold the entries. Names that are not UTF-8 are shown with `\xHH`
# escapes, and such entries are compared by their metadata only: Filer's
# paths are UTF-8.
#
# Usage: lib/filer-ext4/tests/differential.sh, from the repository root,
# on Linux with e2fsprogs, python3, and `sudo` for mounting.

set -eu
export TZ=UTC LC_ALL=C.UTF-8
work=$(mktemp -d)
mnt="$work/mnt"
mkdir "$mnt"
cleanup() {
    mountpoint -q "$mnt" && sudo umount "$mnt"
    rm -rf "$work"
}
trap cleanup EXIT

cargo build -q -p filer-ext4 --example manifest
manifest="$PWD/target/debug/examples/manifest"

# The committed fixtures, and fresh ones.
mkdir "$work/fresh"
lib/filer-ext4/tests/fixtures/make.sh "$work/fresh"
for sparse in lib/filer-ext4/tests/fixtures/*.sparse "$work"/fresh/*.sparse; do
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

# Prints what the Linux kernel shows, in the manifest's format, but for
# the flags and extended attributes.
kernel_manifest() {
    # As root, as `lost+found` is root's alone.
    sudo python3 -I - "$1" <<'PY'
import ctypes, os, re, stat, struct, sys
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
        print(f"{path_shown}\t{kind}\t{st.st_size}\t{times}\t{what}")
        if kind == "d" and not foreign:
            walk(path, path_shown)

walk(b"", "")
PY
}

failed=0
for img in "$work"/*.img; do
    name=$(basename "$img" .img)
    if ! sudo mount -o loop,ro "$img" "$mnt" 2>/dev/null; then
        echo "skipped: $name (Linux cannot mount it here)"
        continue
    fi
    kernel_manifest "$mnt" | sort > "$work/$name.kernel"
    sudo umount "$mnt"
    # The flags and extended attributes are not compared yet.
    "$manifest" "$img" | cut -f 1-8 | sort > "$work/$name.filer"
    if diff -u "$work/$name.kernel" "$work/$name.filer" > "$work/diff"; then
        echo "same: $name ($(wc -l < "$work/$name.filer") entries)"
    else
        echo "DIFFERENT: $name"
        head -40 "$work/diff"
        failed=1
    fi
done
exit $failed
