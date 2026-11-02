#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Tests `filer-xfs` against volumes of other projects' test suites, which
# are not public domain, so are never committed here (see `CREDITS.md`):
# fetches just their fixture files, at the commits named below, into
# `$FILER_XFS_EXTERNAL` (by default `target/xfs-external`), checks them
# by `tests/external.sha256`, unpacks them, and runs `tests/external.rs`.
# On Linux, it then compares the `manifest` example with what the kernel
# shows of each image it mounts, as `tests/differential.sh` does: of a
# copy it recovered, as images may need that, each image a job, as many
# at once as there are cores (or `FILER_JOBS`). None of the projects'
# code is fetched, or to be read.
#
# Usage: lib/filer-xfs/tests/external.sh, from the repository root, with
# git, zstd, and sha256sum; for the comparison, on Linux with python3 and
# `sudo` for mounting.

set -eu
export TZ=UTC LC_ALL=C.UTF-8
repo=$PWD
# shellcheck source=lib/filer-spec/tests/jobs.sh
. "$repo/lib/filer-spec/tests/jobs.sh"
# shellcheck source=lib/filer-xfs/tests/linux.sh
. "$repo/lib/filer-xfs/tests/linux.sh"

# Performs a job: comparing image `$2`, called `$3`, recovered, with
# Linux.
perform() {
    recovered "$2" "$3"
}

if [ "${1:-}" = --job ]; then
    job "$2"
fi

dir=${FILER_XFS_EXTERNAL:-$repo/target/xfs-external}
mkdir -p "$dir"
dir=$(cd "$dir" && pwd)

# Fetches the fixture files of `$4` in `$2`, at commit `$3`, as `$1`.
fetch() {
    if [ -d "$dir/$1" ]; then
        return
    fi
    git init -q "$dir/$1"
    git -C "$dir/$1" remote add origin "$2"
    git -C "$dir/$1" sparse-checkout set --no-cone \
        "/$4/*.xfs" "/$4/*.zst" "/$4/*.tree"
    git -C "$dir/$1" fetch -q --depth 1 --filter=blob:none origin "$3"
    git -C "$dir/$1" checkout -q FETCH_HEAD
}

# Apache-2.0.
fetch go-xfs-filesystem https://github.com/masahiro331/go-xfs-filesystem.git \
    ccf49f5dd0256ebcfc8396a706ffaec8f9739730 xfs/testdata
# BSD-2-Clause.
fetch xfuse https://github.com/KhaledEmaraDev/xfuse.git \
    2ea4e250db00d3f0f845499c8a83f85b9a77b7f8 resources
# MIT or Apache-2.0.
fetch lamxfs https://github.com/lamco-admin/lamxfs.git \
    2881dd0ba90e7b1d32ccac6a2ba3ee57b78792fe tests/fixtures

(cd "$dir" && sha256sum --quiet -c "$repo/lib/filer-xfs/tests/external.sha256")

# Unpacked as sparse files.
mkdir -p "$dir/images"
while read -r _ path; do
    name=$(basename "$path" .zst)
    case $path in
        *.tree) ;;
        *.zst) [ -e "$dir/images/$name" ] ||
            zstd -q -d --sparse "$dir/$path" -o "$dir/images/$name" ;;
        *) [ -e "$dir/images/$name" ] || cp --sparse=always "$dir/$path" "$dir/images/" ;;
    esac
done < "$repo/lib/filer-xfs/tests/external.sha256"

# A release build, as a debug one reads volumes some seventy times slower.
FILER_XFS_EXTERNAL=$dir cargo test -q --release -p filer-xfs --test external -- --ignored

[ "$(uname)" = Linux ] || exit 0
cargo build -q --release -p filer-xfs --example manifest
manifest="$repo/target/release/examples/manifest"
work=$(mktemp -d -p /var/tmp)
trap clean_work EXIT
failed=0
for img in "$dir"/images/*; do
    case $(basename "$img") in
        # Samples of their superblocks only, zoned and realtime volumes,
        # which this driver refuses or reads in part, a torn log, which it
        # does not yet read as Linux does (XFS-15), and a file of more
        # attribute names than Linux lists.
        *_reject.img | xfs_rt1.img | v5_dirty_torn.img | xfs_nrext64.img) continue ;;
    esac
    add recovered "$img" "$(basename "$img")"
done
run
exit $failed
