#!/usr/bin/env node
// Export the sprites named in assets_src/export_ids from assets_src/atlas.svg
// to assets/img/<id>.png, as utils/assets_export.sh does with
// `resvg --export-id`, which current resvg releases render blank for this
// atlas. Each sprite is drawn by rendering the whole atlas with only that
// element visible, through a viewBox cut to the element's bounding box
// (from `resvg --query-all`).
//
//   node utils/export-sprites.mjs [zoom]     (default zoom 3: 384 px sprites)
//
// Needs `resvg` on PATH (cargo install resvg).
import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync, rmSync, mkdtempSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');
const zoom = Number(process.argv[2] || 3);
const atlasPath = path.join(ROOT, 'assets_src', 'atlas.svg');
const atlas = readFileSync(atlasPath, 'utf8');
const ids = readFileSync(path.join(ROOT, 'assets_src', 'export_ids'), 'utf8').split(/\r?\n/).filter(Boolean);
const out = path.join(ROOT, 'assets', 'img');
mkdirSync(out, { recursive: true });

// Bounding boxes in canvas pixels: "id,x,y,w,h" per line.
const boxes = new Map();
for (const line of execFileSync('resvg', ['--query-all', atlasPath], { encoding: 'utf8', maxBuffer: 1 << 26 }).split('\n')) {
  const [id, x, y, w, h] = line.split(',');
  if (id && h) boxes.set(id, [x, y, w, h].map(Number));
}
// Canvas pixels per viewBox unit.
const root = atlas.match(/<svg[^>]*>/)[0];
const vb = root.match(/viewBox="([^"]+)"/)[1].split(/[\s,]+/).map(Number);
const widthMm = Number(root.match(/\swidth="([\d.]+)mm"/)[1]);
const unit = (widthMm * 96 / 25.4) / vb[2];

const work = mkdtempSync(path.join(ROOT, 'assets_src', '.export-'));
try {
  for (const id of ids) {
    const box = boxes.get(id);
    if (!box) throw new Error(`${id}: not in the atlas`);
    const [x, y, w, h] = box;
    const svgTag = root
      .replace(/\swidth="[^"]+"/, ` width="${(w * zoom).toFixed(3)}"`)
      .replace(/\sheight="[^"]+"/, ` height="${(h * zoom).toFixed(3)}"`)
      .replace(/viewBox="[^"]+"/, `viewBox="${vb[0] + x / unit} ${vb[1] + y / unit} ${w / unit} ${h / unit}"`);
    const style = `<style>* { visibility: hidden; } #${id}, #${id} * { visibility: visible; }</style>`;
    const svg = atlas.replace(root, svgTag + style);
    const file = path.join(work, `${id}.svg`);
    writeFileSync(file, svg);
    execFileSync('resvg', [file, path.join(out, `${id}.png`)]);
  }
} finally {
  rmSync(work, { recursive: true, force: true });
}
console.log(`${ids.length} sprites at zoom ${zoom} in ${path.relative(process.cwd(), out)}`);
