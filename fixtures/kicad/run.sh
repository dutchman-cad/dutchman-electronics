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
    python3 /work/fixtures/kicad/fill_zones.py /work/$dir/one-zone.kicad_pcb &&
    python3 /work/fixtures/kicad/fill_zones.py /work/$dir/zone-thermal.kicad_pcb &&
    python3 /work/fixtures/kicad/add_stackup.py /work/$dir/four-layer-stackup.kicad_pcb /work/fixtures/kicad/stackup-four-layer.sexp"
done
# The project files KiCad writes beside each board are not needed.
rm -f "$out"/*.kicad_pro "$out"/*.kicad_prl "$out"/k10-work/*.kicad_pro "$out"/k10-work/*.kicad_prl
for f in "$out"/k10-work/*.kicad_pcb; do
  mv "$f" "$out/k10-$(basename "$f")"
done
rmdir "$out"/k10-work

# KiCad's own Gerber export of the pad boards (KiCad 9), and the polygon pad outlines read from
# it: the reader's pad corners are checked against these.
mkdir -p crates/kicad-pcb/tests/gerber
for b in pad-shapes pad-shapes-turned; do
  podman run --rm --userns=keep-id -v "$PWD:/work:Z" docker.io/kicad/kicad:9.0 \
    kicad-cli pcb export gerbers --layers F.Cu,B.Cu --no-protel-ext -o /work/crates/kicad-pcb/tests/gerber/ /work/$out/$b.kicad_pcb
  rm -f crates/kicad-pcb/tests/gerber/*.gbrjob
  for l in F_Cu B_Cu; do
    python3 fixtures/kicad/gerber_corners.py crates/kicad-pcb/tests/gerber/$b-$l.gbr > crates/kicad-pcb/tests/gerber/$b-$l.corners.txt
  done
done
