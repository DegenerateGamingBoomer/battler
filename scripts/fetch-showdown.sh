#!/usr/bin/env bash
# Fetches and builds the pinned Pokémon Showdown reference under third_party/.
set -euo pipefail

PIN=3661ce40bf9001d185ce078b8920e12304204609
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="$ROOT/third_party/pokemon-showdown"

if [ ! -d "$DEST/.git" ]; then
  mkdir -p "$ROOT/third_party"
  git clone --filter=blob:none https://github.com/smogon/pokemon-showdown.git "$DEST"
fi
git -C "$DEST" fetch --quiet origin "$PIN" 2>/dev/null || true
git -C "$DEST" checkout --quiet "$PIN"
(cd "$DEST" && npm install --no-audit --no-fund && node build)
echo "Showdown $(git -C "$DEST" rev-parse HEAD) built at $DEST"
