#!/bin/sh
# Fills fuzz/corpus/<target>/ with our own test boards, so the fuzzers start from valid files.
# Usage: fuzz/seed.sh [largest-seed-in-bytes]   (default 200000: a mutated big file is slow)
set -eu
here=$(cd "$(dirname "$0")" && pwd)
root=$(dirname "$here")
max=${1:-200000}

put() { # target file
    mkdir -p "$here/corpus/$1"
    size=$(wc -c < "$2")
    [ "$size" -le "$max" ] || return 0
    cp "$2" "$here/corpus/$1/$(basename "$2")"
}

for f in "$root"/crates/altium-pcbdoc/tests/boards/*.PcbDoc; do
    put altium_pcbdoc "$f"
    put package_build "$f"
done
for f in "$root"/crates/altium-pcbdoc/tests/boards/*.PrjPcb \
         "$root"/crates/altium-pcbdoc/tests/boards/*/*.PrjPcb; do
    [ -f "$f" ] && put altium_project "$f"
done
for f in "$root"/crates/kicad-pcb/tests/boards/*.kicad_pcb \
         "$root"/crates/kicad-pcb/tests/open-hardware/*/*.kicad_pcb; do
    put kicad_pcb "$f"
    put package_build "$f"
done

# Fonts: the TrueType files we ship for the social images (all open-licensed).
mkdir -p "$here/corpus/truetype_text"
for f in "$root"/assets/social/fonts/*.ttf; do
    [ -f "$f" ] && cp "$f" "$here/corpus/truetype_text/"
done
