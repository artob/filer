#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Tests `filer-ntfs` against volumes of other projects, which are not kept
# here, as they would only bloat the repository (see `CREDITS.md`):
# fetches just their image files, at the commits named below, into
# `$FILER_NTFS_EXTERNAL` (by default `target/ntfs-external`), checks them
# by `tests/external.sha256`, unpacks each disk's NTFS partition as a
# volume of its own, sparsely, and runs `tests/external.rs`. On Linux, it
# then compares the `manifest` example with what `ntfs3` and ntfs-3g show
# of each volume, as `tests/differential.sh` does, each volume a job, as
# many at once as there are cores (or `FILER_JOBS`).
#
# Usage: lib/filer-ntfs/tests/external.sh, from the repository root, with
# git, python3, tar, and sha256sum; for the comparison, on Linux with
# ntfs-3g, the `ntfs3` module, and `sudo` for mounting.

set -eu
export TZ=UTC LC_ALL=C.UTF-8
repo=$PWD
# shellcheck source=lib/filer-spec/tests/jobs.sh
. "$repo/lib/filer-spec/tests/jobs.sh"
# shellcheck source=lib/filer-ntfs/tests/linux.sh
. "$repo/lib/filer-ntfs/tests/linux.sh"

# Performs a job: comparing volume `$2`, called `$3`, with Linux.
perform() {
    check "$2" "$3"
}

if [ "${1:-}" = --job ]; then
    job "$2"
fi

dir=${FILER_NTFS_EXTERNAL:-$repo/target/ntfs-external}
mkdir -p "$dir"
dir=$(cd "$dir" && pwd)

# Fetches the files of `$2` matching the patterns after `$3`, at commit
# `$3`, as `$1`.
fetch() {
    name=$1 url=$2 commit=$3
    shift 3
    if [ -d "$dir/$name" ]; then
        return
    fi
    git init -q "$dir/$name"
    git -C "$dir/$name" remote add origin "$url"
    git -C "$dir/$name" sparse-checkout set --no-cone "$@"
    git -C "$dir/$name" fetch -q --depth 1 --filter=blob:none origin "$commit"
    git -C "$dir/$name" checkout -q FETCH_HEAD
}

# CC0 1.0, but for the boot code in the images.
fetch ntfs-samples https://github.com/msuhanov/ntfs-samples.git \
    7bdf2b0e61f6d376553fdcbbe8c147b04291c9b8 "/*.gz" "/*.tgz"

(cd "$dir" && sha256sum --quiet -c "$repo/lib/filer-ntfs/tests/external.sha256")

# Writes the first partition of the MBR disk image `$1`, read as gzip if
# its name says so, to volume `$2`, leaving zeros as holes.
partition() {
    python3 -I - "$1" "$2" <<'PY'
import gzip, itertools, os, struct, sys
CHUNK = 1 << 20
path, out = sys.argv[1], sys.argv[2]
def chunks():
    # Pieces of the disk, with their offsets; a plain file's holes are
    # skipped.
    if path.endswith(".gz"):
        with gzip.open(path, "rb") as f:
            at = 0
            while piece := f.read(CHUNK):
                yield at, piece
                at += len(piece)
        return
    fd = os.open(path, os.O_RDONLY)
    size, at = os.fstat(fd).st_size, 0
    try:
        while at < size:
            try:
                at = os.lseek(fd, at, os.SEEK_DATA)
            except OSError:
                return
            end = os.lseek(fd, at, os.SEEK_HOLE)
            while at < end:
                piece = os.pread(fd, min(CHUNK, end - at), at)
                yield at, piece
                at += len(piece)
    finally:
        os.close(fd)
pieces = chunks()
at, first = next(pieces)
assert at == 0 and first[510:512] == b"\x55\xaa", "no MBR"
_, start, sectors = struct.unpack_from("<4xB3xII", first, 446)
start, end = start * 512, (start + sectors) * 512
with open(out, "wb") as f:
    for at, piece in itertools.chain([(0, first)], pieces):
        lo, hi = max(at, start), min(at + len(piece), end)
        piece = piece[lo - at:hi - at]
        if lo < hi and piece.count(0) != len(piece):
            f.seek(lo - start)
            f.write(piece)
    f.truncate(end - start)
PY
}

mkdir -p "$dir/volumes"
for disk in "$dir"/ntfs-samples/*.raw.gz; do
    volume="$dir/volumes/$(basename "$disk" .raw.gz).vol"
    [ -e "$volume" ] || partition "$disk" "$volume"
done
if [ ! -e "$dir/volumes/ntfs.vol" ]; then
    tar -xzf "$dir/ntfs-samples/ntfs.tgz" -C "$dir/ntfs-samples"
    partition "$dir/ntfs-samples/ntfs.raw" "$dir/volumes/ntfs.vol"
    rm "$dir/ntfs-samples/ntfs.raw"
fi

# A release build, as a debug one reads volumes some twenty times slower.
FILER_NTFS_EXTERNAL=$dir cargo test -q --release -p filer-ntfs --test external -- --ignored

[ "$(uname)" = Linux ] || exit 0
cargo build -q --release -p filer-ntfs --example manifest
manifest="$repo/target/release/examples/manifest"
work=$(mktemp -d -p /var/tmp)
trap clean_work EXIT
failed=0
for volume in "$dir"/volumes/*.vol; do
    add volume "$volume" "$(basename "$volume" .vol)"
done
run
exit $failed
