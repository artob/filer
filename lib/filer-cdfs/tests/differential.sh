#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Compares `filer-cdfs` with the Linux kernel's `isofs` driver: for each
# image, the `cdfs-manifest` example's listing must equal what Linux
# shows with the image mounted: names, kinds, sizes, contents, symlink
# targets, and modification times. The images are the committed fixtures
# and fresh ones `tests/fixtures/make.sh` makes, `hidden.iso` too.
#
# Linux is asked to translate no names (`map=off`), and to show Joliet's
# as UTF-8; the names it shows lose their version, `;1`, and the `.`
# before it, as Filer shows them. Linux reads only the primary tree of an
# image of ISO 9660:1999's names, without Rock Ridge's or Joliet's, where
# Filer reads the enhanced one: such images are not compared. Linux looks
# Joliet's names up regardless of case, so that of names differing only
# in case, it finds the first, where Filer matches them exactly: in an
# image of Joliet's names alone, such names are left out.
#
# Listings are compared sorted. Names that are not UTF-8 are shown with
# `\xHH` escapes, and such entries are compared by their metadata only:
# Filer's paths are UTF-8.
#
# Each image is a job of its own, as many at once as there are cores (or
# `FILER_JOBS`), each one's report printed whole, in order.
#
# Usage: lib/filer-cdfs/tests/differential.sh, from the repository root,
# on Linux with `xorriso`, `genisoimage`, python3, and `sudo` for
# mounting.

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
    # As Filer displays names: bytes that are not UTF-8 as `\xHH`, and
    # without a version.
    name = re.sub(rb"\.?;[0-9]*$", b"", name) if re.search(rb";[0-9]*$", name) else name
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

# Leaves out of a listing the paths differing from another only in case.
caseless() {
    python3 -I -c '
import sys
lines = sys.stdin.read().splitlines()
paths = [line.split("\t")[0].lower() for line in lines]
for line, path in zip(lines, paths):
    if paths.count(path) == 1:
        print(line)
'
}

# Compares image `$2`, named `$3`, with what Linux shows of it: the one
# kind of job.
perform() {
    "$manifest" "$2" | sort > "$scratch/filer"
    sudo mount -o loop,ro,map=off,utf8 "$2" "$mnt"
    kernel_manifest "$mnt" | sort > "$scratch/kernel"
    sudo umount "$mnt"
    case $3 in *-rock-ridge-*) ;; *-joliet)
        for side in filer kernel; do
            caseless < "$scratch/$side" > "$scratch/$side.caseless"
            mv "$scratch/$side.caseless" "$scratch/$side"
        done
        ;;
    esac
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
cargo build -q --release -p filer-cdfs --example cdfs-manifest
manifest="$PWD/target/release/examples/cdfs-manifest"

# The committed fixtures, and fresh ones, each a job.
mkdir "$work/fresh"
lib/filer-cdfs/tests/fixtures/make.sh "$work/fresh" >/dev/null 2>&1
lib/filer-cdfs/tests/fixtures/make.sh "$work/fresh" hidden >/dev/null 2>&1
for image in lib/filer-cdfs/tests/fixtures/*.iso "$work"/fresh/*.iso; do
    case $image in */iso9660-1999.iso) continue ;; esac
    case $image in "$work"/*) kind=fresh ;; *) kind=fixture ;; esac
    add image "$image" "$kind-$(basename "$image" .iso)"
done
run
exit $failed
