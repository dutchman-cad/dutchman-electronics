#!/bin/sh
# Rebuilds the test boards in crates/kicad-pcb/tests/boards with KiCad (needs podman): the KiCad 9
# boards under their own names, and the same boards made with KiCad 10 as k10-<name>.
set -e
cd "$(dirname "$0")/../.."
out=crates/kicad-pcb/tests/boards
for v in 9.0 10.0; do
  dir=$out
  [ "$v" = 10.0 ] && dir=$out/k10-work && mkdir -p "$dir"
  podman run --rm --userns=keep-id -v "$PWD:/work:Z" docker.io/kicad/kicad:$v sh -c "
    python3 /work/fixtures/kicad/build_boards.py /work/$dir &&
    python3 /work/fixtures/kicad/fill_zones.py /work/$dir/one-zone.kicad_pcb"
done
# The project files KiCad writes beside each board are not needed.
rm -f "$out"/*.kicad_pro "$out"/*.kicad_prl "$out"/k10-work/*.kicad_pro "$out"/k10-work/*.kicad_prl
for f in "$out"/k10-work/*.kicad_pcb; do
  mv "$f" "$out/k10-$(basename "$f")"
done
rmdir "$out"/k10-work
