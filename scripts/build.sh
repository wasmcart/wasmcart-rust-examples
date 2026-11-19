#!/usr/bin/env bash
# Build every game (or the ones named) and pack each into dist/<name>.wasc.
#
#   scripts/build.sh            # all games
#   scripts/build.sh hexbloom   # one
#
# Needs rustup (rust-toolchain.toml picks stable with the wasm32-unknown-unknown
# and wasm32-wasip1 targets), and Node with this repo's devDependencies
# installed (`npm install`) for wasmcart-pack. Each game's cart.json may name
# its cargo target ("target": "wasm32-wasip1" for the Bevy games); the default
# is wasm32-unknown-unknown.
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
  target="$(node -e 'const c=require(process.argv[1]);console.log(c.target||"wasm32-unknown-unknown")' "$ROOT/games/$g/cart.json")"
  cargo build --release --target "$target" -p "$g"
done
node scripts/pack.mjs "${GAMES[@]}"
