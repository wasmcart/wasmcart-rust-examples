#!/usr/bin/env node
// The cart gate: runs each packed game in dist/ on the wasmcart Node host and
// hands it to the game's own checks (games/<name>/check.mjs), which play it
// with pad input and assert on pixels, motion, debug values and audio, each
// with a control that must fail.
//
//   node scripts/check-carts.mjs [game ...] [--shots <dir>]
import { existsSync, readdirSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import path from 'node:path';
import * as h from './lib/harness.mjs';

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');
const argv = process.argv.slice(2);
const si = argv.indexOf('--shots');
const shots = si >= 0 ? path.resolve(argv[si + 1]) : null;
const named = argv.filter((a, i) => !a.startsWith('--') && (si < 0 || i !== si + 1));
const games = named.length ? named : readdirSync(path.join(ROOT, 'games')).sort();

for (const name of games) {
  const file = path.join(ROOT, 'dist', `${name}.wasc`);
  const checks = path.join(ROOT, 'games', name, 'check.mjs');
  console.log(`\n== ${name}`);
  if (!h.check(`${name}: cart is built`, existsSync(file), file)) continue;
  if (!h.check(`${name}: has checks`, existsSync(checks))) continue;
  const mod = await import(pathToFileURL(checks));
  await mod.default({ ...h, name, file, shots: shots && path.join(shots, name) });
}

const n = h.failures();
console.log(n ? `\n${n} check(s) failed` : '\nall cart checks passed');
process.exit(n ? 1 : 0);
