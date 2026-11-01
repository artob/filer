#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Compares `filer-ntfs` with the Linux kernel's `ntfs3` driver: for each
# image, the `manifest` example's listing must equal what Linux shows with
# the image loop-mounted read-only: names, kinds, sizes, contents, and
# modification, creation, and access times. The images are the committed
# fixtures and fresh ones `tests/fixtures/make.sh` makes.
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
# Filer refuses to). `ntfs3` also reports no birth time for symlinks.
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

failed=0
for img in "$work"/*.img; do
    name=$(basename "$img" .img)
    sudo mount -t ntfs3 -o "loop,ro,iocharset=utf8,uid=$(id -u),gid=$(id -g)" "$img" "$mnt"
    kernel_manifest "$mnt" > "$work/$name.kernel"
    sudo umount "$mnt"
    "$manifest" "$img" > "$work/$name.filer"
    for side in kernel filer; do
        python3 -I - "$work/$name.$side" > "$work/$name.$side.normal" <<'PY'
import sys
# Entries that differ by design, by path (see above).
by_design = {"abs", "junction", "wsl", "cloud.bin", "a/abs"}
for line in open(sys.argv[1], encoding="utf-8"):
    fields = line.rstrip("\n").split("\t")
    if fields[0] in by_design:
        continue
    if fields[1] == "l":
        fields[4] = "-"
    print("\t".join(fields))
PY
    done
    sort "$work/$name.kernel.normal" > "$work/$name.k"
    sort "$work/$name.filer.normal" > "$work/$name.f"
    if diff -u "$work/$name.k" "$work/$name.f" > "$work/$name.diff"; then
        echo "same: $name ($(wc -l < "$work/$name.filer") entries)"
    else
        echo "DIFFERENT: $name"
        head -40 "$work/$name.diff"
        failed=1
    fi
done
exit $failed
