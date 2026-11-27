#!/usr/bin/env bash
# Opens every test board's STEP with the readers people will use: SolidCraft's STEP import and
# cadmpeg. The boards are the Altium ones (crates/altium-pcbdoc/tests/boards/*.PcbDoc), the KiCad
# CC0 ones (crates/kicad-pcb/tests/boards/*.kicad_pcb) and the open-hardware KiCad boards by others
# (crates/kicad-pcb/tests/open-hardware/*/*.kicad_pcb). For each it builds the parts package, then
# requires: cadmpeg `check` OK, and SolidCraft, both on importing the STEP and on reopening the
# design it saved from it, sees one closed body whose box is the board's outline size by its
# thickness to 0.001 mm, with no feature error. The CC0 boards of both formats share a 50 x 30 mm
# outline (see their README.txt) of 1.6 mm thickness (Altium four-layer, four-layer-planes,
# all-features and vias-blind-buried: 1.8122 mm; KiCad four-layer: 1.2 mm); the open-hardware
# boards have the sizes KiCad gives, in their README.txt.
# Each component model the package writes (model-*.step) must pass cadmpeg `check` and, on import
# and on reopening its saved design, be as many closed bodies with a volume as the file has solids
# (one for an extruded body; a STEP model the board embeds may hold several).
#   usage: scripts/check-board-step.sh <dutchman-electronics> <solidcraft-cli> <cadmpeg>
set -euo pipefail
module="$1"; solidcraft="$2"; cadmpeg="$3"
work=$(mktemp -d)
failed=0
boards=(crates/altium-pcbdoc/tests/boards/*.PcbDoc crates/kicad-pcb/tests/boards/*.kicad_pcb
        crates/kicad-pcb/tests/open-hardware/*/*.kicad_pcb)
# Fail loudly if a pattern matched nothing (an unmatched glob stays as its own text).
for board in "${boards[@]}"; do
  [ -e "$board" ] || { echo "::error::no test board at $board"; exit 1; }
done
check_models() {
  local name="$1" dir="$2" model m solids report
  # Each component model the package wrote: cadmpeg check OK, and SolidCraft sees as many bodies
  # as the file has solids, each closed with a volume.
  for model in "$dir"/model-*.step; do
    [ -e "$model" ] || continue
    m=$(basename "$model" .step)
    solids=$(grep -c 'MANIFOLD_SOLID_BREP' "$model" || true)
    if "$cadmpeg" check --json "$model" > "$work/$name.$m.check.json" 2> "$work/$name.$m.check.log" \
      && python3 -c 'import json,sys; sys.exit(json.load(open(sys.argv[1])).get("status") != "ok")' \
        "$work/$name.$m.check.json"; then
      echo "cadmpeg check $m: ok"
    else
      echo "::error::cadmpeg check failed on $name $m"; cat "$work/$name.$m.check.log"; failed=1
    fi
    if ! { "$solidcraft" eval --in "$model" --json-out "$work/$name.$m.solidcraft.json" \
      && "$solidcraft" run --in "$model" --save "$dir/$m.sc" --quiet \
      && "$solidcraft" eval --in "$dir/$m.sc" --json-out "$work/$name.$m.reopened.json"; }; then
      echo "::error::SolidCraft could not open $name $m"; failed=1; continue
    fi
    for report in "$work/$name.$m.solidcraft.json" "$work/$name.$m.reopened.json"; do
    if ! python3 - "$report" "$name $m" "$solids" <<'PY'
import json, sys
report, name, solids = json.load(open(sys.argv[1])), sys.argv[2], int(sys.argv[3])
errors = [t.get("error") for t in report.get("timeline", []) if t.get("error")]
bodies = report.get("bodies", [])
problems = [f"feature error: {e}" for e in errors]
if solids < 1:
    problems.append("the file holds no solid")
if len(bodies) != solids:
    problems.append(f"{len(bodies)} bodies, expected {solids}")
for b in bodies:
    if not b.get("closed"):
        problems.append("a body is not closed")
    if not b.get("volume_mm3", 0) > 0:
        problems.append("a body has no volume")
    print(f"SolidCraft {name} ({sys.argv[1].rsplit('.', 2)[-2]}): closed={b.get('closed')} faces={b.get('faces')} volume={b.get('volume_mm3', 0):.4f} mm3")
if problems:
    print(f"::error::SolidCraft on {name}: " + "; ".join(problems))
    sys.exit(1)
PY
    then failed=1; fi
    done
  done
}
for board in "${boards[@]}"; do
  stem=$(basename "${board%.*}")
  case "$board" in
    *.PcbDoc) name="altium-$stem" ;;
    */open-hardware/*) name="open-$stem" ;;
    *) name="kicad-$stem" ;;
  esac
  dir="$work/$name"
  echo "== $name"
  # The outline's width and height, and the thickness, in mm.
  case "$name" in
    altium-four-layer|altium-four-layer-planes|altium-all-features|altium-vias-blind-buried) width=50; height=30; thickness=1.8122 ;;
    kicad-four-layer|kicad-k10-four-layer) width=50; height=30; thickness=1.2 ;;
    kicad-text-justify) width=96; height=432; thickness=1.6 ;;
    kicad-text-strings) width=96; height=144; thickness=1.6 ;;
    open-GB-CARTPP-XC) width=80; height=55; thickness=1.0 ;;
    open-Franzininho-DIY) width=51.0675; height=30.03; thickness=1.6 ;;
    open-*) echo "::error::no expected size for $name"; failed=1; continue ;;
    *) width=50; height=30; thickness=1.6 ;;
  esac
  "$module" read "$board" --out "$dir" --json > "$work/$name.manifest.json"
  if "$cadmpeg" check --json "$dir/board.step" > "$work/$name.check.json" 2> "$work/$name.check.log" \
    && python3 -c 'import json,sys; sys.exit(json.load(open(sys.argv[1])).get("status") != "ok")' \
      "$work/$name.check.json"; then
    echo "cadmpeg check: ok"
  else
    echo "::error::cadmpeg check failed on $name"; cat "$work/$name.check.log"; failed=1
  fi
  "$solidcraft" eval --in "$dir/board.step" --json-out "$work/$name.solidcraft.json"
  "$solidcraft" run --in "$dir/board.step" --save "$dir/board.sc" --quiet
  "$solidcraft" eval --in "$dir/board.sc" --json-out "$work/$name.reopened.json"
  for report in "$work/$name.solidcraft.json" "$work/$name.reopened.json"; do
  if ! python3 - "$report" "$name" "$width" "$height" "$thickness" <<'PY'
import json, sys
report, name = json.load(open(sys.argv[1])), sys.argv[2]
width, height, thickness = (float(v) for v in sys.argv[3:6])
errors = [t.get("error") for t in report.get("timeline", []) if t.get("error")]
bodies = report.get("bodies", [])
problems = [f"feature error: {e}" for e in errors]
if len(bodies) != 1:
    problems.append(f"{len(bodies)} bodies, expected 1")
for b in bodies:
    if not b.get("closed"):
        problems.append("the body is not closed")
    lo, hi = b["bbox"]["min"], b["bbox"]["max"]
    for axis, want in zip("xyz", (width, height, thickness)):
        i = "xyz".index(axis)
        size = hi[i] - lo[i]
        if abs(size - want) > 0.001:
            problems.append(f"{axis} size {size:.4f} mm, expected {want}")
    print(f"SolidCraft ({sys.argv[1].rsplit('.', 2)[-2]}): closed={b.get('closed')} faces={b.get('faces')} volume={b.get('volume_mm3', 0):.3f} mm3")
if problems:
    print(f"::error::SolidCraft on {name}: " + "; ".join(problems))
    sys.exit(1)
PY
  then failed=1; fi
  done
  check_models "$name" "$dir"
  # The plating (plating.step, when the board has plated holes) holds one tube per hole: cadmpeg
  # check OK, and SolidCraft sees as many bodies as the file has solids, each closed with a volume.
  # altium-one-via and altium-one-plated-pad each have one plated hole, so they must have the file, with 1 tube.
  plating="$dir/plating.step"
  case "$name" in
    altium-one-via|altium-one-plated-pad)
      if [ ! -e "$plating" ]; then
        echo "::error::$name has a plated hole but no plating.step"; failed=1
      elif [ "$(grep -c 'MANIFOLD_SOLID_BREP' "$plating" || true)" != 1 ]; then
        echo "::error::$name plating.step does not hold exactly 1 tube"; failed=1
      fi ;;
  esac
  if [ -e "$plating" ]; then
    tubes=$(grep -c 'MANIFOLD_SOLID_BREP' "$plating" || true)
    if "$cadmpeg" check --json "$plating" > "$work/$name.plating.check.json" 2> "$work/$name.plating.check.log" \
      && python3 -c 'import json,sys; sys.exit(json.load(open(sys.argv[1])).get("status") != "ok")' \
        "$work/$name.plating.check.json"; then
      echo "cadmpeg check plating: ok"
    else
      echo "::error::cadmpeg check failed on $name plating"; cat "$work/$name.plating.check.log"; failed=1
    fi
    if ! { "$solidcraft" eval --in "$plating" --json-out "$work/$name.plating.solidcraft.json" \
      && "$solidcraft" run --in "$plating" --save "$dir/plating.sc" --quiet \
      && "$solidcraft" eval --in "$dir/plating.sc" --json-out "$work/$name.plating.reopened.json"; }; then
      echo "::error::SolidCraft could not open $name plating"; failed=1; continue
    fi
    for report in "$work/$name.plating.solidcraft.json" "$work/$name.plating.reopened.json"; do
    if ! python3 - "$report" "$name plating" "$tubes" <<'PY'
import json, sys
report, name, tubes = json.load(open(sys.argv[1])), sys.argv[2], int(sys.argv[3])
errors = [t.get("error") for t in report.get("timeline", []) if t.get("error")]
bodies = report.get("bodies", [])
problems = [f"feature error: {e}" for e in errors]
if len(bodies) != tubes:
    problems.append(f"{len(bodies)} bodies, expected {tubes}")
for b in bodies:
    if not b.get("closed"):
        problems.append("a body is not closed")
    if not b.get("volume_mm3", 0) > 0:
        problems.append("a body has no volume")
    print(f"SolidCraft {name} ({sys.argv[1].rsplit('.', 2)[-2]}): closed={b.get('closed')} faces={b.get('faces')} volume={b.get('volume_mm3', 0):.6f} mm3")
if problems:
    print(f"::error::SolidCraft on {name}: " + "; ".join(problems))
    sys.exit(1)
PY
    then failed=1; fi
    done
  fi
done
# The standard bodies (`--bodies standard`): the dense board has chip resistors and capacitors,
# SOT-23, SOIC-8, TQFP-32 and 1x4 headers, so the models written for them (a body and its
# leads each) must open as closed solids too, and the package must hold at least one of each kind.
dense="crates/altium-pcbdoc/tests/boards/dense-board.PcbDoc"
dir="$work/altium-dense-board-standard"
echo "== altium-dense-board --bodies standard"
"$module" read "$dense" --bodies standard --out "$dir" --json > "$work/dense-standard.manifest.json"
python3 - "$work/dense-standard.manifest.json" <<'PY' || failed=1
import json, sys
m = json.load(open(sys.argv[1]))
infos = " ".join(n["message"] for n in m["notes"] if n["severity"] == "Info")
leads = [p for p in m["parts"] if p["name"].endswith(" leads")]
missing = [w for w in ("chip resistor", "SOT-23", "SOIC-8", "TQFP-32", "pin header") if w not in infos]
if not leads or missing:
    print(f"::error::dense-board --bodies standard: {len(leads)} lead parts, no note for {missing}")
    sys.exit(1)
print(f"standard bodies: {len(leads)} parts with leads")
PY
check_models "altium-dense-board-standard" "$dir"
exit $failed
