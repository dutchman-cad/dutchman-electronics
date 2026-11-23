#!/usr/bin/env bash
# Fails when a pull request changes reader code or a format description without the ticked
# provenance sentence from LEGAL.md in its description.
#   usage: PR_BODY="..." scripts/check-provenance.sh <base-sha> <head-sha>
set -euo pipefail
base="$1"; head="$2"
# Reader code: every crate's src/ except the board model and the command, which hold no format
# knowledge. Format descriptions: docs/formats/.
touched=$(git diff --name-only "$base" "$head" \
  | grep -E '^(crates/[^/]+/src/|docs/formats/)' \
  | grep -vE '^crates/(board-model|dutchman-electronics)/' || true)
if [ -z "$touched" ]; then
  echo "No reader or format-spec files changed; no provenance sentence needed."
  exit 0
fi
echo "Reader or format-spec files changed:"
echo "$touched" | sed 's/^/  /'
sentence='I derived this contribution solely by analyzing files I am legally entitled to possess and from publicly available information.'
if printf '%s\n' "${PR_BODY:-}" | grep -E '^[[:space:]]*[-*][[:space:]]*\[[xX]\]' | grep -qF "$sentence"; then
  echo "Provenance sentence present and ticked."
  exit 0
fi
echo "::error::This pull request changes reader code or a format description, so its description needs the ticked provenance sentence from LEGAL.md (the first box in the pull-request template). Tick it only if it is true."
exit 1
