#!/usr/bin/env bash
# Opens every CC0 test board's STEP with the readers people will use: SolidCraft's STEP import
# and cadmpeg. For each board under crates/altium-pcbdoc/tests/boards it builds the parts
# package, then requires: cadmpeg `check` OK, and SolidCraft, both on importing the STEP and on
# reopening the design it saved from it, sees one closed body whose box is 50 x 30 x 1.6 mm
# (every test board shares that outline, see its README.txt) to 0.001 mm, with no feature error.
# Each component model the package writes (model-*.step) must pass cadmpeg `check` and, on import
# and on reopening its saved design, be one closed body with a volume.
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
  if ! python3 - "$report" "$name" <<'PY'
import json, sys
report, name = json.load(open(sys.argv[1])), sys.argv[2]
errors = [t.get("error") for t in report.get("timeline", []) if t.get("error")]
bodies = report.get("bodies", [])
problems = [f"feature error: {e}" for e in errors]
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
    print(f"SolidCraft ({sys.argv[1].rsplit('.', 2)[-2]}): closed={b.get('closed')} faces={b.get('faces')} volume={b.get('volume_mm3', 0):.3f} mm3")
if problems:
    print(f"::error::SolidCraft on {name}: " + "; ".join(problems))
    sys.exit(1)
PY
  then failed=1; fi
  done
  # Each component model the package wrote (extruded bodies): cadmpeg check OK, and SolidCraft
  # sees one closed body with a volume.
  for model in "$dir"/model-*.step; do
    [ -e "$model" ] || continue
    m=$(basename "$model" .step)
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
    if ! python3 - "$report" "$name $m" <<'PY'
import json, sys
report, name = json.load(open(sys.argv[1])), sys.argv[2]
errors = [t.get("error") for t in report.get("timeline", []) if t.get("error")]
bodies = report.get("bodies", [])
problems = [f"feature error: {e}" for e in errors]
if len(bodies) != 1:
    problems.append(f"{len(bodies)} bodies, expected 1")
for b in bodies:
    if not b.get("closed"):
        problems.append("the body is not closed")
    if not b.get("volume_mm3", 0) > 0:
        problems.append("the body has no volume")
    print(f"SolidCraft {name} ({sys.argv[1].rsplit('.', 2)[-2]}): closed={b.get('closed')} faces={b.get('faces')} volume={b.get('volume_mm3', 0):.4f} mm3")
if problems:
    print(f"::error::SolidCraft on {name}: " + "; ".join(problems))
    sys.exit(1)
PY
    then failed=1; fi
    done
  done
done
exit $failed
