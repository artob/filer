#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Compares `filer-ntfs` with the Linux kernel's `ntfs3` driver: for each
# image, the `manifest` example's listing must equal what Linux shows with
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
# Filer refuses to). Interix special files (symlinks, FIFOs, sockets, and
# devices) are regular files to `ntfs3`, and are compared with ntfs-3g
# only. `ntfs3` also reports no birth time for symlinks.
#
# Usage: lib/filer-ntfs/tests/differential.sh, from the repository root,
# on Linux with ntfs-3g, python3, GNU coreutils, the `ntfs3` module, and
# `sudo` for mounting.

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

cargo build -q -p filer-ntfs --example manifest
manifest="$PWD/target/debug/examples/manifest"

# The committed fixtures, and fresh ones.
mkdir "$work/fresh"
lib/filer-ntfs/tests/fixtures/make.sh "$work/fresh"
for sparse in lib/filer-ntfs/tests/fixtures/*.sparse "$work"/fresh/*.sparse; do
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
    python3 -I - "$1" <<'PY'
import os, stat, subprocess, sys
root = sys.argv[1]
lines = []

def fnv1a(data):
    h = 0xCBF29CE484222325
    for b in data:
        h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h

def walk(rel):
    # `listdir` returns entries in the order the directory holds them.
    for name in os.listdir(os.path.join(root, rel)):
        path = os.path.join(rel, name)
        full = os.path.join(root, path)
        st = os.lstat(full)
        birth = subprocess.run(["stat", "-c", "%W", "--", full], check=True,
                               capture_output=True, text=True).stdout.strip()
        birth = "-" if birth == "0" else birth
        times = f"{int(st.st_mtime)}\t{birth}\t{int(st.st_atime)}"
        if stat.S_ISLNK(st.st_mode):
            target = os.readlink(full)
            lines.append(f"{path}\tl\t{st.st_size}\t{times}\t{target}")
        elif stat.S_ISDIR(st.st_mode):
            lines.append(f"{path}\td\t-\t{times}\t-")
            walk(path)
        else:
            try:
                data = open(full, "rb").read()
                digest = f"{fnv1a(data):016x}"
            except OSError:
                digest = "!error"
            lines.append(f"{path}\tf\t{st.st_size}\t{times}\t{digest}")

walk("")
print("\n".join(lines))
PY
}

# Prints what ntfs-3g shows: path, kind, flags, and extended attributes,
# in the manifest's format.
ntfs3g_manifest() {
    python3 -I - "$1" <<'PY'
import os, stat, sys
root = sys.argv[1]
# File attributes and the flags Filer reports them as.
FLAGS = [(0x2, "hidden"), (0x4, "system"), (0x20, "archive"),
         (0x200, "sparse"), (0x800, "compressed"), (0x1000, "offline"),
         (0x4000, "encrypted"), (0x40000 | 0x400000, "dataless")]

def fnv1a(data):
    h = 0xCBF29CE484222325
    for b in data:
        h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h

def joined(items):
    return ",".join(sorted(items)) or "-"

def walk(rel):
    for name in os.listdir(os.path.join(root, rel)):
        path = os.path.join(rel, name)
        full = os.path.join(root, path)
        st = os.lstat(full)
        if stat.S_ISLNK(st.st_mode):
            print(f"{path}\tl\t-\t-")
            continue
        bits = int.from_bytes(os.getxattr(full, "system.ntfs_attrib_be"), "big")
        flags = joined(flag for mask, flag in FLAGS if bits & mask)
        xattrs = joined(f"{attr}={fnv1a(os.getxattr(full, attr)):016x}"
                        for attr in os.listxattr(full) if attr.startswith("user."))
        kind = next((letter for test, letter in [
            (stat.S_ISDIR, "d"), (stat.S_ISFIFO, "p"), (stat.S_ISSOCK, "s"),
            (stat.S_ISCHR, "c"), (stat.S_ISBLK, "b")] if test(st.st_mode)), "f")
        print(f"{path}\t{kind}\t{flags}\t{xattrs}")
        if kind == "d":
            walk(path)

walk("")
PY
}

# Normalizes listing `$1`, keeping fields `$2` (0-based, comma-separated),
# into `$1.$3`, sorted: `k` for comparing with `ntfs3`, `x` with ntfs-3g.
normalize() {
    python3 -I - "$1" "$2" "$3" <<'PY' | sort > "$1.$3"
import sys
# Entries that differ by design, by path (see above).
by_design = {"abs", "junction", "wsl", "cloud.bin", "a/abs"}
if sys.argv[3] == "k":
    by_design |= {"ilink", "idirlink", "ichr", "iblk", "fifo", "sock",
                  "special/fifo"}
keep = [int(field) for field in sys.argv[2].split(",")]
for line in open(sys.argv[1], encoding="utf-8"):
    fields = line.rstrip("\n").split("\t")
    if fields[0] in by_design:
        continue
    if fields[1] == "l":
        # Link targets differ in form, and links have no flags here.
        fields = fields[:4] + ["-"] * (len(fields) - 4)
    print("\t".join(fields[i] for i in keep))
PY
}

failed=0
compare() {
    if diff -u "$1" "$2" > "$work/diff"; then
        echo "same: $3 ($(wc -l < "$2") entries)"
    else
        echo "DIFFERENT: $3"
        head -40 "$work/diff"
        failed=1
    fi
}
for img in "$work"/*.img; do
    name=$(basename "$img" .img)
    sudo mount -t ntfs3 -o "loop,ro,iocharset=utf8,uid=$(id -u),gid=$(id -g)" "$img" "$mnt"
    kernel_manifest "$mnt" > "$work/$name.kernel"
    sudo umount "$mnt"
    sudo mount -t ntfs-3g -o "ro,uid=$(id -u),gid=$(id -g)" "$img" "$mnt"
    ntfs3g_manifest "$mnt" > "$work/$name.ntfs3g"
    sudo umount "$mnt"
    "$manifest" "$img" > "$work/$name.filer"
    normalize "$work/$name.kernel" 0,1,2,3,4,5,6 k
    normalize "$work/$name.filer" 0,1,2,3,4,5,6 k
    normalize "$work/$name.ntfs3g" 0,1,2,3 x
    normalize "$work/$name.filer" 0,1,7,8 x
    compare "$work/$name.kernel.k" "$work/$name.filer.k" "$name, ntfs3"
    compare "$work/$name.ntfs3g.x" "$work/$name.filer.x" "$name, ntfs-3g"
done
exit $failed
