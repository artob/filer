#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Runs `filer-windows`'s tests (or those of the packages in
# `$FILER_WINDOWS_PACKAGES`, such as `filer-windows filer-system`) in
# Windows from Linux: builds them for `x86_64-pc-windows-gnu`, which needs
# the MinGW-w64 linker (`gcc-mingw-w64-x86-64`) and `rustup target add
# x86_64-pc-windows-gnu`, then runs each test binary as a job in the
# `filer-windows` container that `lib/filer-ntfs/tests/fixtures/
# make-windows.sh` sets up, and prints what they print. Doctests do not
# run this way.
#
# Usage: lib/filer-windows/tests/windows.sh [test-binary-args...], from the
# repository root; the state directory is `$FILER_WINDOWS`
# (`~/.cache/filer-windows` by default), as for `make-windows.sh`.

set -eu
state=${FILER_WINDOWS:-$HOME/.cache/filer-windows}
shared="$state/shared"
target=x86_64-pc-windows-gnu

docker ps --format '{{.Names}}' | grep -qx filer-windows || docker start filer-windows >/dev/null || {
    echo "no container: run lib/filer-ntfs/tests/fixtures/make-windows.sh first"
    exit 1
}

packages=${FILER_WINDOWS_PACKAGES:-filer-windows}
bins=$(cargo test $(printf -- '-p %s ' $packages) --target "$target" --no-run \
    --message-format=json |
    python3 -Ic '
import json, sys
for line in sys.stdin:
    message = json.loads(line)
    if message.get("reason") == "compiler-artifact" and message.get("executable"):
        print(message["executable"])
')
rm -rf "$shared/tests" "$shared"/job.*
mkdir "$shared/tests"
{
    printf '$share = if (Test-Path Z:\\tests) { "Z:" } else { "\\\\host.lan\\Data" }\r\n'
    printf '$failed = 0\r\n'
    for bin in $bins; do
        name=$(basename "$bin")
        cp "$bin" "$shared/tests/$name"
        printf 'Write-Output "== %s"\r\n' "$name"
        printf '& "$share\\tests\\%s" %s\r\n' "$name" "$*"
        printf 'if ($LASTEXITCODE -ne 0) { $failed = 1 }\r\n'
    done
    printf 'exit $failed\r\n'
} > "$shared/job.tmp"
mv "$shared/job.tmp" "$shared/job.ps1"
echo "waiting for Windows to run the tests"
while [ ! -e "$shared/job.done" ]; do sleep 3; done
sleep 1
tr -d '\r' < "$shared/job.log"
[ "$(tr -d '\r\n' < "$shared/job.done")" = 0 ] || { echo "tests failed"; exit 1; }
