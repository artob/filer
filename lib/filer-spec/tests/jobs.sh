# This is free and unencumbered software released into the public domain.
#
# Running the comparisons of a driver's differential test at once, for the
# scripts that source it, such as `lib/filer-xfs/tests/differential.sh`.
#
# A script lists jobs with `add`, each a kind and up to two arguments, and
# runs them with `run`, which runs the script again as `$0 --job N` for
# each, as many at once as there are cores (or `FILER_JOBS`), then prints
# their reports in order. Run so, the script calls `job N` once it has
# defined `perform`, which `job` calls with the job's kind and arguments,
# in a `scratch` directory and with a mount point `mnt` of its own.
#
# The script sets `work`, a directory of its own; `manifest`, the driver's
# `manifest` example, which jobs get back; and `failed`, which any job that
# fails sets to 1. A job fails by exiting nonzero, or by setting `failed`.

tab=$(printf '\t')

# Adds a job of kind `$1`, with arguments `$2` and `$3`, to the list.
add() {
    printf '%s\t%s\t%s\n' "$1" "${2:-}" "${3:-}" >> "$work/jobs"
}

# Runs the jobs listed, then prints their reports in order.
run() {
    mkdir "$work/out"
    export FILER_TEST_WORK="$work" FILER_TEST_MANIFEST="$manifest"
    count=$(wc -l < "$work/jobs")
    seq "$count" | xargs -P "${FILER_JOBS:-$(nproc)}" -n 1 sh "$0" --job || failed=1
    for n in $(seq "$count"); do
        cat "$work/out/$n"
    done
    rm -r "$work/jobs" "$work/out"
}

# Runs job `$1` of the list: its report goes to `$work/out/$1`.
job() {
    work=$FILER_TEST_WORK
    manifest=$FILER_TEST_MANIFEST
    exec > "$work/out/$1" 2>&1
    scratch=$(mktemp -d -p "$work")
    mnt="$scratch/mnt"
    mkdir "$mnt"
    trap 'if mountpoint -q "$mnt"; then sudo umount "$mnt"; fi; sudo rm -rf "$scratch"' EXIT
    failed=0
    spec=$(sed -n "$1p" "$work/jobs")
    kind=${spec%%"$tab"*}
    rest=${spec#*"$tab"}
    perform "$kind" "${rest%%"$tab"*}" "${rest#*"$tab"}"
    exit $failed
}

# Unmounts what is left mounted in `$work`, and removes it.
clean_work() {
    for mounted in "$work"/mnt "$work"/*/mnt; do
        if mountpoint -q "$mounted"; then sudo umount "$mounted"; fi
    done
    sudo rm -rf "$work"
}
