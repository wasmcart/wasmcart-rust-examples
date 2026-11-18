#!/usr/bin/env node
// Generate Starwell's sounds and music into assets/ (committed). Everything
// is synthesized here, so it is original and CC0.
//
//   node games/starwell/gen-assets.mjs
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  RATE, buf, tone, kick, snare, hat, note, master, fade, loop, lowpass, echo, noise, reseed, mixInto,
} from '../../scripts/lib/synth.mjs';
import { writeWav } from '../../scripts/lib/wav.mjs';

const OUT = path.join(path.dirname(fileURLToPath(import.meta.url)), 'assets');
const save = (name, b) => writeWav(path.join(OUT, name), RATE, b);
const N = (s) => note(s);

// Gate chime: a bright rising arpeggio with a bell tail.
{
  const b = buf(0.7);
  ['E5', 'B5', 'E6'].forEach((n, i) => {
    tone(b, i * 0.045, 0.12, N(n), { wave: 'sine', vol: 0.35, env: { a: 0.003, d: 0.1, s: 0.4, r: 0.35 } });
    tone(b, i * 0.045, 0.1, N(n) + 12, { wave: 'tri', vol: 0.08, env: { a: 0.003, d: 0.06, s: 0.2, r: 0.2 } });
  });
  save('gate.wav', fade(master(echo(b, 0.11, 0.25, 0.25), 0.7), 1, 60));
}

// Crash: a crystalline shatter (noise burst plus falling inharmonic tones).
{
  reseed(42);
  const b = buf(0.9);
  for (let i = 0; i < b.length; i++) {
    const t = i / RATE;
    b[i] += noise() * 0.6 * Math.exp(-t * 9);
  }
  lowpass(b, (i) => 6000 * Math.exp(-i / RATE * 3) + 300);
  for (const [f, v] of [[N('C3'), 0.4], [N('F#4') + 0.3, 0.18], [N('C#6') + 0.4, 0.1], [N('G6') - 0.2, 0.08]]) {
    tone(b, 0, 0.05, f, { wave: 'sine', vol: v, slide: -14, env: { a: 0.001, d: 0.25, s: 0.0, r: 0.4 } });
  }
  kick(b, 0, 0.6);
  save('hit.wav', fade(master(b, 0.9), 1, 80));
}

// Start: a rising swell.
{
  const b = buf(1.0);
  tone(b, 0, 0.6, N('A3'), { wave: 'saw', vol: 0.18, slide: 12, detune: 9, env: { a: 0.25, d: 0.2, s: 0.7, r: 0.3 } });
  tone(b, 0.25, 0.4, N('E5'), { wave: 'sine', vol: 0.25, env: { a: 0.02, d: 0.2, s: 0.5, r: 0.3 } });
  lowpass(b, 4000);
  save('start.wav', fade(master(b, 0.7), 2, 80));
}

// Sector: a short fanfare.
{
  const b = buf(1.6);
  [['C5', 0], ['E5', 0.12], ['G5', 0.24], ['C6', 0.36]].forEach(([n, at]) => {
    tone(b, at, 0.5, N(n), { wave: 'square', vol: 0.09, env: { a: 0.01, d: 0.2, s: 0.5, r: 0.4 } });
    tone(b, at, 0.5, N(n) - 12, { wave: 'tri', vol: 0.2, env: { a: 0.01, d: 0.2, s: 0.6, r: 0.4 } });
  });
  tone(b, 0.48, 0.6, N('G6'), { wave: 'sine', vol: 0.15, vib: 0.2, env: { a: 0.05, d: 0.2, s: 0.6, r: 0.5 } });
  save('sector.wav', fade(master(echo(lowpass(b, 5000), 0.18, 0.3, 0.3), 0.75), 2, 100));
}

// Out of energy: a falling, fading line.
{
  const b = buf(2.0);
  [['G4', 0], ['D#4', 0.3], ['C4', 0.6]].forEach(([n, at], i) => {
    tone(b, at, i === 2 ? 0.9 : 0.25, N(n), { wave: 'tri', vol: 0.3, slide: i === 2 ? -2 : 0, env: { a: 0.01, d: 0.2, s: 0.6, r: 0.4 } });
    tone(b, at, 0.25, N(n) - 12, { wave: 'sine', vol: 0.2, env: { a: 0.01, d: 0.2, s: 0.6, r: 0.4 } });
  });
  save('over.wav', fade(master(echo(b, 0.25, 0.3, 0.3), 0.7), 2, 150));
}

// Pause blip.
{
  const b = buf(0.25);
  tone(b, 0, 0.06, N('A5'), { wave: 'sine', vol: 0.3, env: { a: 0.002, d: 0.05, s: 0.3, r: 0.1 } });
  tone(b, 0.07, 0.06, N('E5'), { wave: 'sine', vol: 0.3, env: { a: 0.002, d: 0.05, s: 0.3, r: 0.1 } });
  save('pause.wav', fade(master(b, 0.6), 1, 30));
}

// Engine hum: a seamless one-second loop of detuned low tones and breath.
{
  reseed(7);
  const b = buf(1.0);
  for (let i = 0; i < b.length; i++) {
    const t = i / RATE;
    // Whole numbers of cycles per second keep the loop seamless.
    b[i] = 0.35 * Math.sin(2 * Math.PI * 55 * t) + 0.2 * Math.sin(2 * Math.PI * 82 * t + 0.4 * Math.sin(2 * Math.PI * 3 * t))
      + 0.1 * Math.sin(2 * Math.PI * 165 * t) + 0.08 * noise();
  }
  lowpass(b, 900);
  // Fold the filter's start-up back so the loop point is clean.
  const head = 400;
  for (let i = 0; i < head; i++) b[i] = b[i] * (i / head) + b[b.length - head + i] * (1 - i / head);
  save('engine.wav', master(b, 0.6));
}

// Title: slow ambient pads over a soft pulse, 8 bars at 84 bpm.
{
  const prog = [['A2', 'E3', 'C4', 'E4'], ['F2', 'C3', 'A3', 'E4'], ['C3', 'G3', 'E4', 'G4'], ['G2', 'D3', 'B3', 'D4']];
  const b = loop(84, 8, ({ out, beat }) => {
    for (let bar = 0; bar < 8; bar++) {
      const ch = prog[bar % 4];
      for (const n of ch) tone(out, beat(bar * 4), beat(4) * 0.95, N(n), { wave: 'tri', vol: 0.07, detune: 7, env: { a: 0.9, d: 0.5, s: 0.8, r: 1.2 } });
      for (let k = 0; k < 8; k++) {
        const n = ch[(k * 3) % 4];
        tone(out, beat(bar * 4 + k * 0.5), 0.18, N(n) + 12, { wave: 'sine', vol: 0.05, env: { a: 0.005, d: 0.15, s: 0.1, r: 0.3 } });
      }
    }
  });
  save('title.wav', master(echo(lowpass(b, 3500), 0.357, 0.35, 0.3), 0.6));
}

// Flight music: driving, 16 bars at 124 bpm.
{
  const prog = [['A2', 'C4', 'E4'], ['F2', 'A3', 'C4'], ['C3', 'E4', 'G4'], ['G2', 'B3', 'D4']];
  const lead = ['E5', 'D5', 'C5', 'B4', 'C5', 'E5', 'A5', 'G5', 'F5', 'E5', 'C5', 'A4', 'G4', 'B4', 'D5', 'B4'];
  const b = loop(124, 16, ({ out, beat }) => {
    for (let bar = 0; bar < 16; bar++) {
      const ch = prog[bar % 4];
      const root = N(ch[0]);
      for (let k = 0; k < 4; k++) {
        kick(out, beat(bar * 4 + k), 0.45);
        if (k % 2 === 1) snare(out, beat(bar * 4 + k), 0.18);
        hat(out, beat(bar * 4 + k + 0.5), 0.07);
      }
      for (let k = 0; k < 8; k++) {
        const n = root + (k % 2 ? 12 : 0);
        tone(out, beat(bar * 4 + k * 0.5), 0.2, n, { wave: 'saw', vol: 0.08, env: { a: 0.005, d: 0.12, s: 0.3, r: 0.06 } });
      }
      for (const n of ch.slice(1)) tone(out, beat(bar * 4), beat(4) * 0.9, N(n), { wave: 'square', vol: 0.025, detune: 8, env: { a: 0.3, d: 0.4, s: 0.7, r: 0.4 } });
      if (bar >= 4) {
        for (let k = 0; k < 2; k++) {
          const n = lead[((bar - 4) * 2 + k) % lead.length];
          tone(out, beat(bar * 4 + k * 2), beat(1.6), N(n), { wave: 'pulse25', vol: 0.05, vib: 0.15, env: { a: 0.02, d: 0.3, s: 0.5, r: 0.3 } });
        }
      }
    }
  });
  save('music.wav', master(lowpass(b, 6000), 0.6));
}
console.log('starwell assets written to', OUT);
