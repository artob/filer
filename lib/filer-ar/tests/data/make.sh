#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Makes the ar fixtures with real tools: GNU `ar` (`gnu.a`), LLVM's
# `llvm-ar` in its BSD and Windows formats (`bsd.a`, `coff.lib`), all
# from two small objects and a text file or two, and `dpkg-deb` a Debian
# package with uncompressed tarballs (`hello.deb`). `darwin.a` comes from
# macOS's `ar`, which keeps only Mach-O objects and always adds a symbol
# table:
#
#     cc -Os -c hello.c a_rather_long_object_name.c
#     touch -t 202311142213.20 *.o && ar rc darwin.a hello.o \
#         a_rather_long_object_name.o
#
# Usage: tests/data/make.sh [output-dir], on Linux with binutils, gcc,
# llvm, and dpkg.

set -eu
out=$(cd "${1:-$(dirname "$0")}" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work"
printf 'int hello(void) { return 1; }\n' > hello.c
printf 'int a_rather_long_function_name(void) { return 2; }\n' \
    > a_rather_long_object_name.c
gcc -Os -c hello.c a_rather_long_object_name.c
printf 'Hello, world!\n' > notes.txt
printf 'spaced\n' > 'with space.txt'
touch -d @1700000000 hello.o a_rather_long_object_name.o notes.txt \
    'with space.txt'
objects="hello.o a_rather_long_object_name.o notes.txt"
rm -f "$out/gnu.a" "$out/bsd.a" "$out/coff.lib" "$out/hello.deb"
# `U` keeps the files' times and owners.
ar rcsU "$out/gnu.a" $objects
llvm-ar rcsU --format=bsd "$out/bsd.a" $objects 'with space.txt'
llvm-ar rcsU --format=coff "$out/coff.lib" $objects

mkdir -p hello/DEBIAN hello/usr/share/doc/hello
cat > hello/DEBIAN/control <<EOF
Package: hello
Version: 1.0
Architecture: all
Maintainer: Filer <filer@example.com>
Description: Says hello
EOF
printf 'Hello, world!\n' > hello/usr/share/doc/hello/README
find hello -exec touch -d @1700000000 {} +
SOURCE_DATE_EPOCH=1700000000 dpkg-deb --root-owner-group -Znone \
    --build hello "$out/hello.deb" >/dev/null
