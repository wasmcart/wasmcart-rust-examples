#!/usr/bin/env bash
# The gate: generate nothing, build every game, then run every game's checks
# on the wasmcart Node host (real GL, real WebGPU on a core device and in
# compatibility mode).
#
#   scripts/test.sh [game ...]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
# GPU work stays on the low-power GPU of a two-GPU machine.
export WASMCART_WGPU_POWER="${WASMCART_WGPU_POWER:-low-power}"

echo "== build"
scripts/build.sh "$@"
echo "== carts on the Node host"
node scripts/check-carts.mjs "$@" --shots shots
