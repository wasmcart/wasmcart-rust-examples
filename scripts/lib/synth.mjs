// A small deterministic synthesizer for the games' sound effects and music.
// Everything is generated from code, so every sound in these carts is
// original and CC0. Mono Float32 buffers; mix() sums them.

export const RATE = 32000;

let seed = 0x1234567;
/** Deterministic noise in -1..1 (a fixed LCG, so the assets are reproducible). */
export function noise() {
  seed = (Math.imul(seed, 1103515245) + 12345) >>> 0;
  return (seed >>> 8) / 0x7fffff - 1;
}
export function reseed(s) { seed = s >>> 0; }

/** MIDI note to Hz. */
export const hz = (n) => 440 * Math.pow(2, (n - 69) / 12);

/** Note names like 'C4', 'F#3', 'Bb2' to MIDI numbers. */
export function note(name) {
  const m = /^([A-G])([#b]?)(-?\d)$/.exec(name);
  if (!m) throw new Error(`bad note ${name}`);
  const base = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 }[m[1]];
  return 12 * (Number(m[3]) + 1) + base + (m[2] === '#' ? 1 : m[2] === 'b' ? -1 : 0);
}

export const osc = {
  sine: (p) => Math.sin(2 * Math.PI * p),
  tri: (p) => 1 - 4 * Math.abs((p % 1) - 0.5),
  saw: (p) => 2 * (p % 1) - 1,
  square: (p) => ((p % 1) < 0.5 ? 1 : -1),
  pulse25: (p) => ((p % 1) < 0.25 ? 1 : -1),
};

/** A buffer of `secs` seconds. */
export const buf = (secs) => new Float32Array(Math.ceil(secs * RATE));

/** Attack/decay/sustain/release envelope value at time t for a note held `len` s. */
export function adsr(t, len, { a = 0.005, d = 0.08, s = 0.6, r = 0.08 } = {}) {
  if (t < 0) return 0;
  if (t < a) return t / a;
  if (t < a + d) return 1 - (1 - s) * ((t - a) / d);
  if (t < len) return s;
  const rt = t - len;
  return rt < r ? s * (1 - rt / r) : 0;
}

/** One-pole low-pass in place; cutoff in Hz (constant or function of sample index). */
export function lowpass(b, cutoff) {
  let y = 0;
  for (let i = 0; i < b.length; i++) {
    const fc = typeof cutoff === 'function' ? cutoff(i) : cutoff;
    const k = 1 - Math.exp(-2 * Math.PI * fc / RATE);
    y += k * (b[i] - y);
    b[i] = y;
  }
  return b;
}

/**
 * Render a tone into `out` starting at `at` seconds.
 * opts: wave, vol, env, slide (semitones over the note), vib (depth semis), detune (cents, adds a 2nd voice)
 */
export function tone(out, at, len, midi, { wave = 'square', vol = 0.2, env, slide = 0, vib = 0, vibRate = 5.5, detune = 0 } = {}) {
  const fn = osc[wave];
  const rel = (env && env.r) ?? 0.08;
  const start = Math.round(at * RATE);
  const n = Math.round((len + rel) * RATE);
  let p1 = 0, p2 = 0;
  for (let i = 0; i < n && start + i < out.length; i++) {
    const t = i / RATE;
    const semis = midi + slide * Math.min(1, t / len) + vib * Math.sin(2 * Math.PI * vibRate * t) * Math.min(1, t * 4);
    const f = hz(semis);
    p1 += f / RATE;
    let v = fn(p1);
    if (detune) { p2 += (f * Math.pow(2, detune / 1200)) / RATE; v = 0.5 * (v + fn(p2)); }
    if (start + i >= 0) out[start + i] += v * vol * adsr(t, len, env);
  }
}

/** Kick drum at `at` s. */
export function kick(out, at, vol = 0.7) {
  const start = Math.round(at * RATE), n = Math.round(0.25 * RATE);
  let p = 0;
  for (let i = 0; i < n && start + i < out.length; i++) {
    const t = i / RATE;
    p += (45 + 110 * Math.exp(-t * 30)) / RATE;
    out[start + i] += vol * Math.sin(2 * Math.PI * p) * Math.exp(-t * 9);
  }
}

/** Snare: noise burst plus a short body tone. */
export function snare(out, at, vol = 0.35) {
  const start = Math.round(at * RATE), n = Math.round(0.2 * RATE);
  let lp = 0;
  for (let i = 0; i < n && start + i < out.length; i++) {
    const t = i / RATE;
    const nz = noise();
    lp += 0.5 * (nz - lp);
    out[start + i] += vol * ((nz - lp) * 0.8 * Math.exp(-t * 18) + 0.4 * Math.sin(2 * Math.PI * 190 * t) * Math.exp(-t * 25));
  }
}

/** Closed hi-hat: high-passed noise tick. */
export function hat(out, at, vol = 0.12, decay = 60) {
  const start = Math.round(at * RATE), n = Math.round(0.06 * RATE);
  let lp = 0;
  for (let i = 0; i < n && start + i < out.length; i++) {
    const t = i / RATE;
    const nz = noise();
    lp += 0.3 * (nz - lp);
    out[start + i] += vol * (nz - lp) * Math.exp(-t * decay);
  }
}

/** Sum `b` into `out` at `at` seconds with gain. */
export function mixInto(out, b, at = 0, gain = 1) {
  const s = Math.round(at * RATE);
  for (let i = 0; i < b.length && s + i < out.length; i++) out[s + i] += b[i] * gain;
  return out;
}

/** Scale so the peak is `peak`; soft-clip with tanh first. */
export function master(b, peak = 0.85) {
  let m = 0;
  for (let i = 0; i < b.length; i++) { b[i] = Math.tanh(b[i] * 1.2); m = Math.max(m, Math.abs(b[i])); }
  if (m > 0) for (let i = 0; i < b.length; i++) b[i] *= peak / m;
  return b;
}

/** Short fades at both ends (in ms), for one-shot sounds. */
export function fade(b, inMs = 2, outMs = 20) {
  const a = Math.round(inMs * RATE / 1000), z = Math.round(outMs * RATE / 1000);
  for (let i = 0; i < a && i < b.length; i++) b[i] *= i / a;
  for (let i = 0; i < z && i < b.length; i++) b[b.length - 1 - i] *= i / z;
  return b;
}

/**
 * A loop that wraps seamlessly: anything rendered past the end (release
 * tails) is folded back onto the start.
 */
export function wrapTail(b, tail) {
  for (let i = 0; i < tail.length; i++) b[i % b.length] += tail[i];
  return b;
}

/** Feedback echo in place. */
export function echo(b, delaySecs = 0.3, fb = 0.3, wet = 0.3) {
  const d = Math.round(delaySecs * RATE);
  const line = new Float32Array(b.length);
  for (let i = 0; i < b.length; i++) {
    const del = i >= d ? line[i - d] : 0;
    line[i] = b[i] + del * fb;
    b[i] += del * wet;
  }
  return b;
}

/**
 * Render a music loop of `bars` bars at `bpm` (4/4) with a callback that
 * schedules notes: song(ctx) where ctx = { out, beat(n) -> seconds }.
 * Release tails past the loop end wrap around so the loop is seamless.
 */
export function loop(bpm, bars, song) {
  const beatSecs = 60 / bpm;
  const len = bars * 4 * beatSecs;
  const out = buf(len + 2);
  song({ out, beat: (n) => n * beatSecs, beatSecs });
  const main = out.slice(0, Math.round(len * RATE));
  return wrapTail(main, out.slice(main.length));
}
