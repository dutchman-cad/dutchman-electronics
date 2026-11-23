#!/usr/bin/env bash
# Opens every CC0 test board's STEP with the readers people will use: SolidCraft's STEP import
# and cadmpeg. For each board under crates/altium-pcbdoc/tests/boards it builds the parts
# package, then requires: cadmpeg `check` OK, and SolidCraft sees one closed body whose box is
# 50 x 30 x 1.6 mm (every test board shares that outline, see its README.txt) to 0.001 mm.
#   usage: scripts/check-board-step.sh <dutchman-electronics> <solidcraft-cli> <cadmpeg>
set -euo pipefail
module="$1"; solidcraft="$2"; cadmpeg="$3"
work=$(mktemp -d)
failed=0
for board in crates/altium-pcbdoc/tests/boards/*.PcbDoc; do
  name=$(basename "$board" .PcbDoc)
  dir="$work/$name"
  echo "== $name"
  "$module" read "$board" --out "$dir" --json > "$work/$name.manifest.json"
  if ! "$cadmpeg" check "$dir/board.step" > "$work/$name.check.txt" 2>&1; then
    echo "::error::cadmpeg check failed on $name"; cat "$work/$name.check.txt"; failed=1
  else
    grep -m1 "^Check:" "$work/$name.check.txt" || head -1 "$work/$name.check.txt"
  fi
  "$solidcraft" eval --in "$dir/board.step" --json-out "$work/$name.solidcraft.json"
  if ! python3 - "$work/$name.solidcraft.json" "$name" <<'PY'
import json, sys
report, name = json.load(open(sys.argv[1])), sys.argv[2]
bodies = report.get("bodies", [])
problems = []
if len(bodies) != 1:
    problems.append(f"{len(bodies)} bodies, expected 1")
for b in bodies:
    if not b.get("closed"):
        problems.append("the body is not closed")
    lo, hi = b["bbox"]["min"], b["bbox"]["max"]
    for axis, want in zip("xyz", (50.0, 30.0, 1.6)):
        i = "xyz".index(axis)
        size = hi[i] - lo[i]
        if abs(size - want) > 0.001:
            problems.append(f"{axis} size {size:.4f} mm, expected {want}")
    print(f"SolidCraft: closed={b.get('closed')} faces={b.get('faces')} volume={b.get('volume_mm3', 0):.3f} mm3")
if problems:
    print(f"::error::SolidCraft on {name}: " + "; ".join(problems))
    sys.exit(1)
PY
  then failed=1; fi
done
exit $failed
