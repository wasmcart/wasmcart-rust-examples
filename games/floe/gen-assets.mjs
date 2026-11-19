#!/usr/bin/env node
// Generates every sound in Bumper Floe from code (CC0): effects and the
// music loops. Run with node; writes assets/*.wav.
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import {
  RATE, buf, tone, kick, snare, hat, note, noise, reseed, master, fade, echo, loop,
} from '../../scripts/lib/synth.mjs';
import { writeWav } from '../../scripts/lib/wav.mjs';

const OUT = path.join(path.dirname(fileURLToPath(import.meta.url)), 'assets');
const save = (name, b) => writeWav(path.join(OUT, `${name}.wav`), RATE, b);

/** Noise into `out` with an amplitude envelope env(t) and a low-pass cutoff(t). */
function noiseBurst(out, at, len, env, cutoff) {
  const s = Math.round(at * RATE), n = Math.round(len * RATE);
  let y = 0;
  for (let i = 0; i < n && s + i < out.length; i++) {
    const t = i / RATE;
    const k = 1 - Math.exp(-2 * Math.PI * cutoff(t) / RATE);
    y += k * (noise() - y);
    out[s + i] += y * env(t);
  }
}

reseed(42);

// bump: a rubbery thud.
{
  const b = buf(0.3);
  tone(b, 0, 0.12, note('A2'), { wave: 'sine', vol: 0.9, slide: -12, env: { a: 0.002, d: 0.1, s: 0.2, r: 0.08 } });
  tone(b, 0, 0.06, note('E4'), { wave: 'tri', vol: 0.25, slide: -10, env: { a: 0.001, d: 0.05, s: 0.1, r: 0.04 } });
  noiseBurst(b, 0, 0.08, (t) => 0.5 * Math.exp(-t * 50), () => 1800);
  save('bump', fade(master(b, 0.8)));
}

// dash: a whoosh rising out of the charge.
{
  const b = buf(0.35);
  noiseBurst(b, 0, 0.32, (t) => Math.sin(Math.PI * t / 0.32) * 0.9, (t) => 600 + 5000 * t);
  tone(b, 0, 0.2, note('C4'), { wave: 'tri', vol: 0.15, slide: 12, env: { a: 0.01, d: 0.1, s: 0.3, r: 0.08 } });
  save('dash', fade(master(b, 0.6)));
}

// splash: a heavy plop then bubbling.
{
  const b = buf(0.9);
  noiseBurst(b, 0, 0.7, (t) => Math.exp(-t * 6) * (t < 0.01 ? t / 0.01 : 1), (t) => 3000 * Math.exp(-t * 4) + 200);
  tone(b, 0, 0.12, note('G3'), { wave: 'sine', vol: 0.6, slide: -14, env: { a: 0.002, d: 0.1, s: 0.2, r: 0.05 } });
  for (let i = 0; i < 7; i++) {
    tone(b, 0.15 + i * 0.08 + Math.abs(noise()) * 0.04, 0.04, note('C5') + Math.round(noise() * 6), { wave: 'sine', vol: 0.12, slide: 7, env: { a: 0.003, d: 0.03, s: 0.2, r: 0.02 } });
  }
  save('splash', fade(master(b, 0.8)));
}

// crack: a dry crackle (ice starting to split).
{
  const b = buf(0.5);
  for (let i = 0; i < 9; i++) {
    noiseBurst(b, i * 0.045 + Math.abs(noise()) * 0.02, 0.03, (t) => 0.8 * Math.exp(-t * 120), () => 5000);
  }
  tone(b, 0, 0.3, note('D3'), { wave: 'saw', vol: 0.08, slide: -5, env: { a: 0.01, d: 0.2, s: 0.2, r: 0.1 } });
  save('crack', fade(master(b, 0.55)));
}

// break: a chunk of ice falls away.
{
  const b = buf(0.9);
  noiseBurst(b, 0, 0.5, (t) => Math.exp(-t * 7), (t) => 1200 * Math.exp(-t * 3) + 150);
  tone(b, 0, 0.3, note('C2'), { wave: 'sine', vol: 0.7, slide: -5, env: { a: 0.005, d: 0.25, s: 0.2, r: 0.1 } });
  noiseBurst(b, 0.2, 0.6, (t) => 0.4 * Math.exp(-t * 5), () => 2500);
  save('break', fade(master(b, 0.7)));
}

// join: two quick rising notes.
{
  const b = buf(0.3);
  tone(b, 0, 0.07, note('E5'), { wave: 'pulse25', vol: 0.3, env: { a: 0.002, d: 0.05, s: 0.5, r: 0.04 } });
  tone(b, 0.08, 0.12, note('B5'), { wave: 'pulse25', vol: 0.3, env: { a: 0.002, d: 0.06, s: 0.5, r: 0.06 } });
  save('join', fade(master(b, 0.6)));
}

// puck: a bright pickup arpeggio.
{
  const b = buf(0.5);
  ['C6', 'E6', 'G6', 'C7'].forEach((n, i) => tone(b, i * 0.05, 0.08, note(n), { wave: 'sine', vol: 0.35, env: { a: 0.002, d: 0.05, s: 0.4, r: 0.12 } }));
  save('puck', fade(master(b, 0.6)));
}

// tick and go: the round countdown.
{
  const b = buf(0.2);
  tone(b, 0, 0.08, note('A5'), { wave: 'square', vol: 0.25, env: { a: 0.002, d: 0.05, s: 0.4, r: 0.05 } });
  save('tick', fade(master(b, 0.45)));
  const g = buf(0.5);
  tone(g, 0, 0.3, note('A6'), { wave: 'square', vol: 0.2, env: { a: 0.002, d: 0.1, s: 0.5, r: 0.12 } });
  tone(g, 0, 0.3, note('E6'), { wave: 'square', vol: 0.15, env: { a: 0.002, d: 0.1, s: 0.5, r: 0.12 } });
  save('go', fade(master(g, 0.55)));
}

// round: a short winner jingle.
{
  const b = buf(1.1);
  [['C5', 0], ['E5', 0.12], ['G5', 0.24], ['C6', 0.36]].forEach(([n, at]) =>
    tone(b, at, n === 'C6' ? 0.4 : 0.1, note(n), { wave: 'pulse25', vol: 0.25, env: { a: 0.003, d: 0.06, s: 0.6, r: 0.15 } }));
  tone(b, 0.36, 0.4, note('C4'), { wave: 'tri', vol: 0.4, env: { a: 0.003, d: 0.1, s: 0.6, r: 0.2 } });
  save('round', fade(master(echo(b, 0.15, 0.25, 0.25), 0.65)));
}

// match: the champion fanfare.
{
  const b = buf(2.6);
  const mel = [['G4', 0, 0.12], ['C5', 0.15, 0.12], ['E5', 0.3, 0.12], ['G5', 0.45, 0.3], ['E5', 0.8, 0.12], ['G5', 0.95, 0.12], ['C6', 1.1, 0.9]];
  for (const [n, at, len] of mel) {
    tone(b, at, len, note(n), { wave: 'square', vol: 0.18, detune: 8, env: { a: 0.004, d: 0.08, s: 0.7, r: 0.2 } });
  }
  for (const [n, at] of [['C3', 0], ['G3', 0.45], ['C3', 1.1]]) tone(b, at, 0.35, note(n), { wave: 'tri', vol: 0.45 });
  for (const n of ['C4', 'E4', 'G4']) tone(b, 1.1, 1.0, note(n), { wave: 'saw', vol: 0.06, env: { a: 0.05, d: 0.3, s: 0.6, r: 0.4 } });
  kick(b, 0); kick(b, 0.45); kick(b, 1.1);
  save('match', fade(master(echo(b, 0.2, 0.3, 0.3), 0.7)));
}

// music: a bouncy loop for play (8 bars at 132 bpm).
{
  const prog = [['C3', ['E4', 'G4', 'C5']], ['A2', ['E4', 'A4', 'C5']], ['F2', ['F4', 'A4', 'C5']], ['G2', ['D4', 'G4', 'B4']]];
  const melody = [
    'G5', null, 'E5', 'G5', 'A5', null, 'G5', 'E5', 'C5', null, 'D5', 'E5', 'D5', null, null, null,
    'E5', null, 'C5', 'E5', 'G5', null, 'A5', 'G5', 'F5', null, 'E5', 'D5', 'B4', null, 'D5', null,
  ];
  const m = loop(132, 8, ({ out, beat }) => {
    for (let bar = 0; bar < 8; bar++) {
      const [bass, chord] = prog[bar % 4];
      for (let q = 0; q < 4; q++) {
        const at = beat(bar * 4 + q);
        kick(out, at, q % 2 === 0 ? 0.55 : 0.3);
        if (q % 2 === 1) snare(out, at, 0.22);
        hat(out, at + beat(0.5), 0.07);
        tone(out, at, beat(0.4), note(bass) + (q === 3 ? 7 : 0), { wave: 'tri', vol: 0.32, env: { a: 0.004, d: 0.1, s: 0.6, r: 0.05 } });
        for (const c of chord) tone(out, at + beat(0.5), beat(0.25), note(c), { wave: 'pulse25', vol: 0.045, env: { a: 0.003, d: 0.05, s: 0.4, r: 0.04 } });
      }
      for (let e = 0; e < 8; e++) {
        const n = melody[((bar % 4) * 8 + e) % melody.length];
        if (n && bar >= 2) tone(out, beat(bar * 4 + e * 0.5), beat(0.42), note(n), { wave: 'square', vol: 0.07, vib: 0.15, env: { a: 0.004, d: 0.08, s: 0.6, r: 0.06 } });
      }
    }
  });
  save('music', master(m, 0.6));
}

// lobby: a calmer loop for the title and lobby (4 bars at 96 bpm).
{
  const chords = [['C4', 'E4', 'G4', 'B4'], ['A3', 'C4', 'E4', 'G4'], ['F3', 'A3', 'C4', 'E4'], ['G3', 'B3', 'D4', 'F4']];
  const m = loop(96, 4, ({ out, beat }) => {
    for (let bar = 0; bar < 4; bar++) {
      const ch = chords[bar];
      for (const c of ch) tone(out, beat(bar * 4), beat(3.6), note(c), { wave: 'tri', vol: 0.06, env: { a: 0.3, d: 0.5, s: 0.7, r: 0.4 } });
      tone(out, beat(bar * 4), beat(1.8), note(ch[0]) - 12, { wave: 'sine', vol: 0.3, env: { a: 0.01, d: 0.3, s: 0.5, r: 0.2 } });
      tone(out, beat(bar * 4 + 2), beat(1.8), note(ch[0]) - 12, { wave: 'sine', vol: 0.25, env: { a: 0.01, d: 0.3, s: 0.5, r: 0.2 } });
      for (let e = 0; e < 8; e++) {
        const c = ch[(e * 3) % 4];
        tone(out, beat(bar * 4 + e * 0.5), beat(0.3), note(c) + 12, { wave: 'sine', vol: 0.07, env: { a: 0.003, d: 0.2, s: 0.2, r: 0.2 } });
      }
      hat(out, beat(bar * 4 + 1), 0.04); hat(out, beat(bar * 4 + 3), 0.04);
    }
  });
  save('lobby', master(echo(m, 0.31, 0.3, 0.25), 0.55));
}

console.log('wrote', OUT);
