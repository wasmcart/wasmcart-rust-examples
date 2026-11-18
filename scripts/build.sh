#!/usr/bin/env bash
# Build every game (or the ones named) and pack each into dist/<name>.wasc.
#
#   scripts/build.sh            # all games
#   scripts/build.sh hexbloom   # one
#
# Needs cargo with the wasm32-unknown-unknown target, and Node with this
# repo's devDependencies installed (`npm install`) for wasmcart-pack.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [ "$#" -gt 0 ]; then
  GAMES=("$@")
else
  GAMES=()
  for d in games/*/; do GAMES+=("$(basename "$d")"); done
fi

# One cargo run per game: building several together would unify their
# features across games that do not share them.
for g in "${GAMES[@]}"; do
  cargo build --release --target wasm32-unknown-unknown -p "$g"
done
node scripts/pack.mjs "${GAMES[@]}"
