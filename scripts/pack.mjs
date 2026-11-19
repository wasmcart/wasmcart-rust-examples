#!/usr/bin/env node
// Pack built games into dist/<name>.wasc with wasmcart-pack. The cart's
// version is its crate's (or cart.json's "version"); title, description,
// author, players and controls come from the game's cart.json, its launcher
// art from games/<name>/art.
// cart.json may also name the cargo target ("target") and the size.
//
//   node scripts/pack.mjs hexbloom tetherline
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');
const pack = path.join(ROOT, 'node_modules', '.bin', 'wasmcart-pack');
if (!existsSync(pack)) {
  console.error('wasmcart-pack missing: run npm install');
  process.exit(1);
}
mkdirSync(path.join(ROOT, 'dist'), { recursive: true });

for (const name of process.argv.slice(2)) {
  const dir = path.join(ROOT, 'games', name);
  const toml = readFileSync(path.join(dir, 'Cargo.toml'), 'utf8');
  const c = JSON.parse(readFileSync(path.join(dir, 'cart.json'), 'utf8'));
  // A vendored port keeps its upstream crate version; cart.json's version
  // then counts the cart's own releases.
  const version = c.version || toml.match(/^version\s*=\s*"([^"]+)"/m)[1];
  // A bin crate builds <name>.wasm, a cdylib <name with - as _>.wasm.
  const rel = path.join(process.env.CARGO_TARGET_DIR || path.join(ROOT, 'target'), c.target || 'wasm32-unknown-unknown', 'release');
  const wasm = [`${name}.wasm`, `${name.replace(/-/g, '_')}.wasm`].map((f) => path.join(rel, f)).find(existsSync);
  if (!wasm) throw new Error(`${name}: no wasm in ${rel}; build it first`);
  const out = path.join(ROOT, 'dist', `${name}.wasc`);
  const args = ['--wasm', wasm, '--name', name, '--version', version, '-o', out];
  if (existsSync(path.join(dir, 'assets'))) args.push('--assets', path.join(dir, 'assets'));
  if (existsSync(path.join(dir, 'art'))) args.push('--art', path.join(dir, 'art'));
  for (const k of ['title', 'description', 'author', 'controls']) if (c[k]) args.push(`--${k}`, c[k]);
  if (c.players) args.push('--players', String(c.players));
  if (c.width && c.height) args.push('--width', String(c.width), '--height', String(c.height));
  execFileSync(pack, args, { stdio: ['ignore', 'ignore', 'inherit'] });
  console.log(`${name.padEnd(12)} wasm ${String(statSync(wasm).size).padStart(8)} bytes   wasc ${String(statSync(out).size).padStart(8)} bytes`);
}
