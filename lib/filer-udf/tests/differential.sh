#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Compares `filer-udf` with the Linux kernel's `udf` driver: for each
# volume, the `udf-manifest` example's listing must equal what Linux
# shows with the volume mounted: names, kinds, sizes, contents, symlink
# targets, and modification times. The volumes are the committed fixtures
# and fresh ones `tests/fixtures/make.sh` makes, of 2 KiB blocks, a
# CD-RW's sparable partition, and a CD-R's virtual one too.
#
# Listings are compared sorted. Each volume is a job of its own, as many
# at once as there are cores (or `FILER_JOBS`), each one's report printed
# whole, in order.
#
# Usage: lib/filer-udf/tests/differential.sh, from the repository root, on
# Linux with `udftools`, python3, and `sudo` for mounting.

set -eu
export TZ=UTC LC_ALL=C.UTF-8
# shellcheck source=lib/filer-spec/tests/jobs.sh
. lib/filer-spec/tests/jobs.sh

# Prints what Linux shows of the tree at `$1`, in the manifest's format.
kernel_manifest() {
    sudo python3 -I - "$1" <<'PY'
import os, re, stat, sys
root = os.fsencode(sys.argv[1])

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

KINDS = [(stat.S_ISDIR, "d"), (stat.S_ISLNK, "l"), (stat.S_ISFIFO, "p"),
         (stat.S_ISSOCK, "s"), (stat.S_ISCHR, "c"), (stat.S_ISBLK, "b")]

def walk(rel, rel_shown):
    for name in os.listdir(os.path.join(root, rel)):
        path = os.path.join(rel, name)
        full = os.path.join(root, path)
        path_shown = shown(name) if not rel else f"{rel_shown}/{shown(name)}"
        st = os.lstat(full)
        kind = next((k for test, k in KINDS if test(st.st_mode)), "f")
        size = st.st_size if kind in "fdl" else 0
        mtime = f"{st.st_mtime_ns // 10**9}.{st.st_mtime_ns % 10**9:09}"
        foreign = "\\x" in shown(name)
        if foreign or kind in "pscbd":
            what = "-"
        elif kind == "l":
            what = os.readlink(full).decode("utf-8", "replace")
        else:
            what = f"{fnv1a(open(full, 'rb').read()):016x}"
        print(f"{path_shown}\t{kind}\t{size}\t{mtime}\t{what}")
        if kind == "d" and not foreign:
            walk(path, path_shown)

walk(b"", "")
PY
}

# Expands sparse fixture `$1` into volume `$2`.
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

# Compares the volume of sparse fixture `$2`, named `$3`, with what Linux
# shows of it: the one kind of job.
perform() {
    volume="$scratch/volume.img"
    unsparse "$2" "$volume"
    "$manifest" "$volume" | sort > "$scratch/filer"
    sudo mount -t udf -o loop,ro,iocharset=utf8 "$volume" "$mnt"
    kernel_manifest "$mnt" | sort > "$scratch/kernel"
    sudo umount "$mnt"
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

# A release build, as a debug one reads many times slower.
cargo build -q --release -p filer-udf --example udf-manifest
manifest="$PWD/target/release/examples/udf-manifest"

# The committed fixtures, and fresh ones, each a job.
mkdir "$work/fresh"
lib/filer-udf/tests/fixtures/make.sh "$work/fresh" >/dev/null 2>&1
lib/filer-udf/tests/fixtures/make.sh "$work/fresh" udf-2048 udf-cdrw udf-cdr >/dev/null 2>&1
for sparse in lib/filer-udf/tests/fixtures/*.sparse "$work"/fresh/*.sparse; do
    case $sparse in "$work"/*) kind=fresh ;; *) kind=fixture ;; esac
    add image "$sparse" "$kind-$(basename "$sparse" .sparse)"
done
run
exit $failed
