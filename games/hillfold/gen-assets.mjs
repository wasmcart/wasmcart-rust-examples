#!/usr/bin/env node
// Generate Hillfold's sounds and music into assets/ (committed). Everything
// is synthesized here, so it is original and CC0.
//
//   node games/hillfold/gen-assets.mjs
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  RATE, buf, tone, kick, hat, note, master, fade, loop, lowpass, echo, noise, reseed,
} from '../../scripts/lib/synth.mjs';
import { writeWav } from '../../scripts/lib/wav.mjs';

const OUT = path.join(path.dirname(fileURLToPath(import.meta.url)), 'assets');
const save = (name, b) => writeWav(path.join(OUT, name), RATE, b);
const N = (s) => note(s);

/** Two resonant band filters over a buzzy source: a voice-like formant. */
function formant(b, f1, f2, q = 0.06) {
  const out = new Float32Array(b.length);
  for (const [f, g] of [[f1, 1], [f2, 0.6]]) {
    // a simple two-pole resonator
    const w = 2 * Math.PI * f / RATE;
    const r = 1 - q;
    const a1 = 2 * r * Math.cos(w), a2 = -r * r;
    let y1 = 0, y2 = 0;
    for (let i = 0; i < b.length; i++) {
      const y = b[i] * (1 - r) + a1 * y1 + a2 * y2;
      y2 = y1; y1 = y;
      out[i] += y * g;
    }
  }
  return out;
}

// Bark: a short buzzy chirp through a mouth-like formant, two takes.
for (const [name, f0, len, seed] of [['bark.wav', 330, 0.16, 3], ['bark2.wav', 300, 0.14, 4]]) {
  reseed(seed);
  const b = buf(len + 0.15);
  let p = 0;
  for (let i = 0; i < b.length; i++) {
    const t = i / RATE;
    const f = f0 * (1 + 0.5 * Math.exp(-t * 30)) * (1 - 0.25 * t / len);
    p += f / RATE;
    const env = t < 0.008 ? t / 0.008 : Math.exp(-(t - 0.008) * 16);
    b[i] = ((p % 1) * 2 - 1 + noise() * 0.5) * env;
  }
  save(name, fade(master(formant(b, 900, 1900, 0.05), 0.85), 1, 40));
}

// Baa: a wobbling nasal bleat, three voices.
for (const [name, f0, len, seed] of [['baa1.wav', 260, 0.7, 11], ['baa2.wav', 300, 0.55, 12], ['baa3.wav', 220, 0.8, 13]]) {
  reseed(seed);
  const b = buf(len + 0.2);
  let p = 0;
  for (let i = 0; i < b.length; i++) {
    const t = i / RATE;
    // the bleat's tremolo: about 18 Hz wobble in pitch and loudness
    const wob = Math.sin(2 * Math.PI * 17 * t);
    const f = f0 * (1 + 0.035 * wob) * (1 - 0.08 * t / len);
    p += f / RATE;
    const env = Math.min(1, t / 0.05) * (t < len ? 1 : Math.max(0, 1 - (t - len) / 0.15)) * (0.75 + 0.25 * wob);
    b[i] = ((p % 1) * 2 - 1 + noise() * 0.15) * env;
  }
  save(name, fade(master(formant(b, 700, 1500, 0.04), 0.6), 5, 60));
}

// Into the fold: a soft wooden bell.
{
  const b = buf(0.9);
  tone(b, 0, 0.04, N('A5'), { wave: 'sine', vol: 0.4, env: { a: 0.002, d: 0.3, s: 0.0, r: 0.5 } });
  tone(b, 0, 0.04, N('E6') + 0.2, { wave: 'sine', vol: 0.15, env: { a: 0.002, d: 0.15, s: 0.0, r: 0.3 } });
  tone(b, 0, 0.04, N('A4'), { wave: 'tri', vol: 0.15, env: { a: 0.002, d: 0.2, s: 0.0, r: 0.3 } });
  save('fold.wav', fade(master(b, 0.6), 1, 60));
}

// Gate: a wooden clack and a latch.
{
  reseed(21);
  const b = buf(0.5);
  for (const at of [0, 0.12]) {
    const s = Math.round(at * RATE);
    for (let i = 0; i < 0.08 * RATE; i++) {
      const t = i / RATE;
      b[s + i] += (noise() * 0.6 + Math.sin(2 * Math.PI * (at ? 520 : 310) * t)) * Math.exp(-t * 60);
    }
  }
  save('gate.wav', fade(master(lowpass(b, 3000), 0.8), 1, 40));
}

// Field clear: a bright folk fanfare.
{
  const b = buf(2.2);
  [['G4', 0], ['B4', 0.13], ['D5', 0.26], ['G5', 0.39], ['B5', 0.6], ['G5', 0.78]].forEach(([n, at], i) => {
    tone(b, at, i >= 4 ? 0.5 : 0.14, N(n), { wave: 'tri', vol: 0.3, env: { a: 0.01, d: 0.15, s: 0.6, r: 0.4 } });
    tone(b, at, i >= 4 ? 0.5 : 0.14, N(n) + 12, { wave: 'sine', vol: 0.08, vib: 0.1, env: { a: 0.02, d: 0.15, s: 0.5, r: 0.4 } });
  });
  for (const [n, at] of [['G3', 0], ['D3', 0.39], ['G2', 0.78]]) tone(b, at, 0.35, N(n), { wave: 'sine', vol: 0.3, env: { a: 0.01, d: 0.2, s: 0.5, r: 0.3 } });
  save('clear.wav', fade(master(echo(b, 0.2, 0.25, 0.25), 0.75), 2, 150));
}

// Night falls: a slow falling line.
{
  const b = buf(2.4);
  [['E4', 0], ['C4', 0.4], ['A3', 0.8]].forEach(([n, at], i) => {
    tone(b, at, i === 2 ? 1.0 : 0.35, N(n), { wave: 'tri', vol: 0.3, slide: i === 2 ? -1 : 0, env: { a: 0.02, d: 0.3, s: 0.6, r: 0.5 } });
    tone(b, at, 0.35, N(n) - 12, { wave: 'sine', vol: 0.2, env: { a: 0.02, d: 0.3, s: 0.6, r: 0.5 } });
  });
  save('fail.wav', fade(master(echo(lowpass(b, 2500), 0.3, 0.3, 0.3), 0.7), 2, 200));
}

// Start: a whistle, two notes up.
{
  const b = buf(0.8);
  tone(b, 0, 0.16, N('D6'), { wave: 'sine', vol: 0.4, slide: 2, env: { a: 0.02, d: 0.1, s: 0.8, r: 0.05 } });
  tone(b, 0.22, 0.28, N('A6'), { wave: 'sine', vol: 0.4, slide: -1, vib: 0.2, env: { a: 0.02, d: 0.1, s: 0.8, r: 0.15 } });
  save('start.wav', fade(master(b, 0.6), 2, 80));
}

// Pause blip.
{
  const b = buf(0.25);
  tone(b, 0, 0.06, N('G5'), { wave: 'sine', vol: 0.3, env: { a: 0.002, d: 0.05, s: 0.3, r: 0.1 } });
  tone(b, 0.07, 0.06, N('D5'), { wave: 'sine', vol: 0.3, env: { a: 0.002, d: 0.05, s: 0.3, r: 0.1 } });
  save('pause.wav', fade(master(b, 0.55), 1, 30));
}

// Dusk: a distant church-like bell, twice.
{
  const b = buf(3.0);
  for (const at of [0, 1.1]) {
    for (const [r, v, d] of [[1, 0.35, 1.2], [2.0, 0.15, 0.8], [2.76, 0.1, 0.6], [5.4, 0.04, 0.3]]) {
      tone(b, at, 0.02, N('D4') + 12 * Math.log2(r), { wave: 'sine', vol: v, env: { a: 0.003, d, s: 0.0, r: d } });
    }
  }
  save('dusk.wav', fade(master(echo(b, 0.37, 0.3, 0.3), 0.6), 2, 300));
}

// Victory: the folk tune's ending, with drums.
{
  const b = buf(3.6);
  const mel = [['D5', 0, 0.2], ['E5', 0.22, 0.2], ['G5', 0.44, 0.2], ['B5', 0.66, 0.4], ['A5', 1.1, 0.2], ['G5', 1.32, 0.2], ['D6', 1.54, 1.2]];
  for (const [n, at, l] of mel) {
    tone(b, at, l, N(n), { wave: 'tri', vol: 0.28, vib: 0.12, env: { a: 0.01, d: 0.2, s: 0.7, r: 0.4 } });
    tone(b, at, l, N(n) - 12, { wave: 'pulse25', vol: 0.05, env: { a: 0.01, d: 0.2, s: 0.5, r: 0.3 } });
  }
  for (const [n, at] of [['G3', 0], ['C4', 0.66], ['D4', 1.1], ['G3', 1.54]]) {
    for (const k of [0, 4, 7]) tone(b, at, 0.5, N(n) + k, { wave: 'tri', vol: 0.06, env: { a: 0.02, d: 0.3, s: 0.6, r: 0.4 } });
    kick(b, at, 0.35);
  }
  save('win.wav', fade(master(echo(b, 0.22, 0.25, 0.25), 0.75), 2, 300));
}

// A plucked string: a decaying tone with a bright attack.
function pluck(out, at, len, midi, vol = 0.12) {
  tone(out, at, 0.02, midi, { wave: 'saw', vol, env: { a: 0.002, d: len, s: 0.0, r: len * 0.5 } });
  tone(out, at, 0.02, midi + 12, { wave: 'tri', vol: vol * 0.4, env: { a: 0.002, d: len * 0.4, s: 0.0, r: len * 0.2 } });
}
// A soft wooden flute: a sine with breath and a little vibrato.
function flute(out, at, len, midi, vol = 0.12) {
  tone(out, at, len, midi, { wave: 'sine', vol, vib: 0.12, vibRate: 5, env: { a: 0.06, d: 0.1, s: 0.85, r: 0.15 } });
  tone(out, at, len, midi + 12, { wave: 'sine', vol: vol * 0.12, env: { a: 0.06, d: 0.1, s: 0.8, r: 0.1 } });
}

// Title: a calm pastoral, plucked chords and a slow flute, 8 bars at 76 bpm.
{
  const prog = [['G2', 'D3', 'G3', 'B3'], ['C3', 'G3', 'C4', 'E4'], ['E2', 'B2', 'E3', 'G3'], ['D3', 'A3', 'D4', 'F#4']];
  const tune = [['B4', 0, 2], ['D5', 2, 2], ['E5', 4, 3], ['D5', 7, 1], ['B4', 8, 2], ['A4', 10, 2], ['G4', 12, 4],
    ['B4', 16, 2], ['D5', 18, 2], ['G5', 20, 3], ['F#5', 23, 1], ['E5', 24, 2], ['D5', 26, 2], ['A4', 28, 4]];
  const b = loop(76, 8, ({ out, beat }) => {
    for (let bar = 0; bar < 8; bar++) {
      const ch = prog[bar % 4];
      for (let k = 0; k < 8; k++) pluck(out, beat(bar * 4 + k * 0.5), 0.5, N(ch[[0, 2, 1, 3, 2, 1, 3, 2][k]]), 0.07);
    }
    for (const [n, at, l] of tune) flute(out, beat(at), beat(l) * 0.95, N(n), 0.13);
  });
  save('title.wav', master(echo(lowpass(b, 4000), 0.395, 0.3, 0.25), 0.6));
}

// Field music: a brisk folk jig feel, 16 bars at 112 bpm.
{
  const prog = [['G2', 'G3', 'B3', 'D4'], ['C3', 'C4', 'E4', 'G4'], ['D3', 'D4', 'F#4', 'A4'], ['G2', 'G3', 'B3', 'D4'],
    ['E3', 'E4', 'G4', 'B4'], ['C3', 'C4', 'E4', 'G4'], ['A2', 'A3', 'C4', 'E4'], ['D3', 'D4', 'F#4', 'A4']];
  const lead = ['G4', 'A4', 'B4', 'D5', 'E5', 'D5', 'B4', 'A4', 'G4', 'B4', 'D5', 'G5', 'F#5', 'E5', 'D5', 'A4'];
  const b = loop(112, 16, ({ out, beat }) => {
    for (let bar = 0; bar < 16; bar++) {
      const ch = prog[bar % 8];
      // bass on 1 and 3, chord strums on the offbeats
      tone(out, beat(bar * 4), 0.35, N(ch[0]), { wave: 'tri', vol: 0.22, env: { a: 0.005, d: 0.2, s: 0.4, r: 0.1 } });
      tone(out, beat(bar * 4 + 2), 0.35, N(ch[0]) + 7, { wave: 'tri', vol: 0.18, env: { a: 0.005, d: 0.2, s: 0.4, r: 0.1 } });
      for (const k of [1, 3]) for (const n of ch.slice(1)) pluck(out, beat(bar * 4 + k) + 0.01 * ch.indexOf(n), 0.3, N(n), 0.04);
      kick(out, beat(bar * 4), 0.25);
      kick(out, beat(bar * 4 + 2), 0.18);
      for (let k = 0; k < 8; k++) hat(out, beat(bar * 4 + k * 0.5), k % 2 ? 0.035 : 0.05);
      if (bar >= 4) {
        // the fiddle-ish lead: three notes a bar, the last held
        const base = ((bar - 4) * 3) % lead.length;
        for (let k = 0; k < 3; k++) {
          const n = N(lead[(base + k) % lead.length]);
          const at = beat(bar * 4 + [0, 1, 2][k]);
          const l = k === 2 ? beat(1.9) : beat(0.9);
          tone(out, at, l, n, { wave: 'saw', vol: 0.045, vib: 0.15, vibRate: 6, env: { a: 0.03, d: 0.1, s: 0.8, r: 0.1 } });
          flute(out, at, l, n + 12, 0.035);
        }
      }
    }
  });
  save('music.wav', master(lowpass(b, 5000), 0.6));
}
console.log('hillfold assets written to', OUT);
