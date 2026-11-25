#!/bin/sh
# Rebuilds the test boards in crates/kicad-pcb/tests/boards with KiCad 9 (needs podman).
set -e
cd "$(dirname "$0")/../.."
out=crates/kicad-pcb/tests/boards
podman run --rm --userns=keep-id -v "$PWD:/work:Z" docker.io/kicad/kicad:9.0 sh -c "
  python3 /work/fixtures/kicad/build_boards.py /work/$out &&
  python3 /work/fixtures/kicad/fill_zones.py /work/$out/one-zone.kicad_pcb"
# The project files KiCad writes beside each board are not needed.
rm -f "$out"/*.kicad_pro "$out"/*.kicad_prl
