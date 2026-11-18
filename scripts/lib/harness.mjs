// The shared cart-check harness: boot a packed cart on the wasmcart Node
// host (CartHost: real GL through webgl-node, real WebGPU through
// webgpu-node), drive its pads, read its frames, debug values and audio.
//
// Every check has a control that must FAIL. A control that passes means the
// check cannot tell a working cart from a broken one, and fails the gate.
import { writeFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { CartHost } from 'wasmcart';
import { encodePng } from './png.mjs';

export const BTN = {
  A: 1 << 0, B: 1 << 1, X: 1 << 2, Y: 1 << 3, L: 1 << 4, R: 1 << 5,
  START: 1 << 6, SELECT: 1 << 7, UP: 1 << 8, DOWN: 1 << 9, LEFT: 1 << 10, RIGHT: 1 << 11,
};

let failed = 0;
export const failures = () => failed;

/** Print one check. */
export function check(name, ok, detail = '') {
  console.log(`${ok ? 'ok  ' : 'FAIL'}  ${name}${detail ? '   ' + detail : ''}`);
  if (!ok) failed++;
  return ok;
}
/** A control must fail: `wouldPass` true means the check is blind. */
export function control(name, wouldPass, detail = '') {
  return check(`control: ${name} is rejected`, !wouldPass, detail);
}

/**
 * A running cart.
 * @param {string} file .wasc path
 * @param {{ featureLevel?: 'core'|'compatibility', wgpu?: boolean }} [opts]
 */
export async function boot(file, opts = {}) {
  const host = new CartHost();
  const load = {};
  if (opts.featureLevel) load.adapterOptions = { featureLevel: opts.featureLevel };
  if (opts.wgpu === false) load.wgpu = false;
  await host.load(file, load);
  host.setFixedStep(1000 / 60);
  const audio = [];
  const cart = {
    host,
    frame: 0,
    /** Run `n` frames holding `pads` (array of pad objects or a function of the frame). */
    async run(n, pads = []) {
      for (let i = 0; i < n; i++) {
        const p = typeof pads === 'function' ? pads(cart.frame, cart) : pads;
        const fr = host.runFrame(p);
        if (fr && fr.audio) audio.push(Float32Array.from(fr.audio));
        cart.frame++;
        if (host.usesWgpu) {
          if (host.gpuLost) throw new Error(`GPU lost: ${JSON.stringify(host.gpuLost)}`);
          await new Promise((r) => setImmediate(r));
        }
      }
    },
    /** A named debug value, or null when the cart has none by that name. */
    v(name) {
      try { return host.readDebugValue(name).value; } catch { return null; }
    },
    /** The current frame as top-down RGBA {w, h, data}. */
    async grab() {
      if (host.usesWgpu) {
        const g = await host.readGpuFrame();
        return { w: g.width, h: g.height, data: Uint8Array.from(g.data) };
      }
      const gl = host.getGlContext();
      const { width: w, height: h } = host.getInfo();
      const raw = new Uint8Array(w * h * 4);
      const read = () => gl.readPixels(0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, raw);
      if (!host.withRenderedFrame(read)) read();
      const out = new Uint8Array(w * h * 4);
      for (let y = 0; y < h; y++) out.set(raw.subarray((h - 1 - y) * w * 4, (h - y) * w * 4), y * w * 4);
      return { w, h, data: out };
    },
    /** RMS of the audio the cart has produced so far (and clear it). */
    audioRms() {
      let s = 0, n = 0;
      for (const c of audio) for (const x of c) { s += x * x; n++; }
      audio.length = 0;
      return { rms: n ? Math.sqrt(s / n) : 0, samples: n };
    },
    log() {
      try { return host.drainDebugEvents().log.map((l) => l.text); } catch { return []; }
    },
    destroy() { host.destroy?.(); },
  };
  return cart;
}

/** One pad: `buttons` mask plus optional sticks in -1..1. */
export function pad(buttons = 0, { lx = 0, ly = 0, rx = 0, ry = 0 } = {}) {
  const ax = (v) => Math.max(-32768, Math.min(32767, Math.round(v * 32767)));
  return { connected: true, buttons, left_x: ax(lx), left_y: ax(ly), right_x: ax(rx), right_y: ax(ry) };
}
/** A connected pad nobody touches (the idle second seat). */
export const idle = () => pad(0);

export const px = (f, x, y) => Array.from(f.data.subarray((y * f.w + x) * 4, (y * f.w + x) * 4 + 3));
export const near = (a, b, tol = 24) => a.every((v, i) => Math.abs(v - b[i]) <= tol);

/** Pixels that differ by more than a small threshold between two frames. */
export function changed(a, b) {
  let n = 0;
  for (let i = 0; i < a.data.length; i += 4) {
    if (Math.abs(a.data[i] - b.data[i]) + Math.abs(a.data[i + 1] - b.data[i + 1]) + Math.abs(a.data[i + 2] - b.data[i + 2]) > 30) n++;
  }
  return n;
}

/** Count pixels matching pred(r,g,b) inside box. */
export function countWhere(f, pred, box = { x: 0, y: 0, w: f.w, h: f.h }) {
  let n = 0;
  for (let y = box.y; y < box.y + box.h; y++) {
    for (let x = box.x; x < box.x + box.w; x++) {
      const i = (y * f.w + x) * 4;
      if (pred(f.data[i], f.data[i + 1], f.data[i + 2])) n++;
    }
  }
  return n;
}

/** Number of distinct colours (quantized to 4 bits a channel). */
export function colours(f) {
  const s = new Set();
  for (let i = 0; i < f.data.length; i += 4) s.add(((f.data[i] >> 4) << 8) | ((f.data[i + 1] >> 4) << 4) | (f.data[i + 2] >> 4));
  return s.size;
}

/** A black frame the size of f, for controls. */
export const black = (f) => ({ w: f.w, h: f.h, data: new Uint8Array(f.data.length) });

/** Save a frame as PNG into dir (no-op without a dir). */
export function savePng(dir, file, f) {
  if (!dir) return;
  mkdirSync(dir, { recursive: true });
  writeFileSync(path.join(dir, file), encodePng(f.w, f.h, f.data));
}
