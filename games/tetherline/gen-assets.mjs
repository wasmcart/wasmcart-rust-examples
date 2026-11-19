#!/usr/bin/env node
// Tetherline's sounds and music, generated from code (original, CC0).
// Re-run after editing; the WAVs are committed.
//
//   node games/tetherline/gen-assets.mjs
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { writeWav } from '../../scripts/lib/wav.mjs';
import * as s from '../../scripts/lib/synth.mjs';

const OUT = path.join(path.dirname(fileURLToPath(import.meta.url)), 'assets');
const save = (name, b) => writeWav(path.join(OUT, name), s.RATE, b);
const R = s.RATE;

// jump: a quick upward chirp.
{
  const b = s.buf(0.16);
  s.tone(b, 0, 0.1, 64, { wave: 'square', vol: 0.18, slide: 12, env: { a: 0.002, d: 0.08, s: 0.3, r: 0.05 } });
  s.lowpass(b, 3500);
  save('jump.wav', s.fade(b));
}
// tether: a zip as the line flies out.
{
  const b = s.buf(0.18);
  for (let i = 0; i < b.length; i++) { const t = i / R; b[i] = s.noise() * 0.4 * Math.exp(-t * 14); }
  s.lowpass(b, (i) => 600 + 6000 * (i / b.length));
  s.tone(b, 0, 0.12, 76, { wave: 'tri', vol: 0.2, slide: 14, env: { a: 0.002, d: 0.1, s: 0.2, r: 0.05 } });
  save('tether.wav', s.fade(b));
}
// clip: the hook catches a ring.
{
  const b = s.buf(0.15);
  s.tone(b, 0, 0.02, 91, { wave: 'square', vol: 0.18, env: { a: 0.001, d: 0.02, s: 0.1, r: 0.03 } });
  s.tone(b, 0.025, 0.05, 98, { wave: 'sine', vol: 0.3, env: { a: 0.001, d: 0.05, s: 0.2, r: 0.06 } });
  save('clip.wav', s.fade(b));
}
// whoosh: letting go at speed.
{
  const b = s.buf(0.4);
  for (let i = 0; i < b.length; i++) { const t = i / R; b[i] = s.noise() * 0.5 * Math.sin(Math.PI * t / 0.4); }
  s.lowpass(b, (i) => 400 + 2500 * Math.sin(Math.PI * i / b.length));
  save('whoosh.wav', s.fade(b));
}
// shard: a sparkling pickup.
{
  const b = s.buf(0.5);
  [84, 88, 91, 96].forEach((n, k) => s.tone(b, k * 0.05, 0.08, n, { wave: 'sine', vol: 0.28, env: { a: 0.002, d: 0.06, s: 0.4, r: 0.2 } }));
  s.echo(b, 0.11, 0.3, 0.25);
  save('shard.wav', s.fade(b));
}
// spark: the small pickups, a single soft tick.
{
  const b = s.buf(0.12);
  s.tone(b, 0, 0.04, 93, { wave: 'tri', vol: 0.22, env: { a: 0.001, d: 0.04, s: 0.2, r: 0.06 } });
  save('spark.wav', s.fade(b));
}
// checkpoint: two warm notes.
{
  const b = s.buf(0.7);
  s.tone(b, 0, 0.15, 67, { wave: 'pulse25', vol: 0.15, env: { a: 0.005, d: 0.1, s: 0.5, r: 0.2 } });
  s.tone(b, 0.14, 0.3, 74, { wave: 'pulse25', vol: 0.15, env: { a: 0.005, d: 0.1, s: 0.5, r: 0.25 } });
  s.echo(b, 0.15, 0.25, 0.25);
  save('checkpoint.wav', s.fade(b));
}
// fall: a soft descending tumble.
{
  const b = s.buf(0.7);
  s.tone(b, 0, 0.5, 72, { wave: 'tri', vol: 0.3, slide: -24, env: { a: 0.005, d: 0.3, s: 0.5, r: 0.2 } });
  for (let i = 0; i < b.length; i++) b[i] += 0.12 * s.noise() * Math.exp(-i / R * 6);
  s.lowpass(b, 2500);
  save('fall.wav', s.fade(s.master(b, 0.7)));
}
// flag: the level is done, a little rising tune.
{
  const b = s.buf(1.6);
  [[67, 0, 0.12], [71, 0.12, 0.12], [74, 0.24, 0.12], [79, 0.36, 0.5]].forEach(([n, t, l]) => {
    s.tone(b, t, l, n, { wave: 'pulse25', vol: 0.14, detune: 6, env: { a: 0.005, d: 0.1, s: 0.6, r: 0.3 } });
    s.tone(b, t, l, n - 12, { wave: 'tri', vol: 0.18, env: { a: 0.005, d: 0.1, s: 0.6, r: 0.3 } });
  });
  s.echo(b, 0.2, 0.3, 0.3);
  save('flag.wav', s.fade(b));
}
// music: an easygoing loop in G major, 104 bpm, 8 bars, with a whistled tune.
{
  const m = s.loop(104, 8, ({ out, beat }) => {
    const prog = [[55, 59, 62], [52, 55, 59], [48, 52, 55], [50, 54, 57], [55, 59, 62], [52, 55, 59], [48, 52, 55], [50, 54, 57]];
    const melody = [
      [74, 0, 1], [76, 1, 0.5], [74, 1.5, 0.5], [71, 2, 1.5],
      [69, 4, 1], [71, 5, 0.5], [72, 5.5, 0.5], [74, 6, 2],
      [76, 8, 1], [74, 9, 0.5], [72, 9.5, 0.5], [71, 10, 1], [67, 11, 1],
      [69, 12, 1.5], [71, 13.5, 0.5], [69, 14, 2],
    ];
    for (let bar = 0; bar < 8; bar++) {
      const ch = prog[bar];
      s.tone(out, beat(bar * 4), beat(1.6), ch[0] - 12, { wave: 'tri', vol: 0.22, env: { a: 0.01, d: 0.2, s: 0.6, r: 0.1 } });
      s.tone(out, beat(bar * 4 + 2), beat(1.6), ch[0] - 5, { wave: 'tri', vol: 0.18, env: { a: 0.01, d: 0.2, s: 0.6, r: 0.1 } });
      // plucked chord on the off-beats
      for (const q of [1, 3]) for (const n of ch) s.tone(out, beat(bar * 4 + q), beat(0.4), n, { wave: 'square', vol: 0.03, env: { a: 0.003, d: 0.15, s: 0.2, r: 0.1 } });
      for (let q = 0; q < 4; q++) {
        s.hat(out, beat(bar * 4 + q + 0.5), 0.04, 80);
        if (q === 0 || q === 2) s.kick(out, beat(bar * 4 + q), 0.3);
      }
    }
    for (const half of [0, 16]) {
      for (const [n, b0, l] of melody) {
        s.tone(out, beat(half + b0), beat(l) * 0.9, n + (half ? 0 : 0), { wave: 'sine', vol: 0.12, vib: 0.15, env: { a: 0.03, d: 0.1, s: 0.8, r: 0.12 } });
      }
    }
  });
  s.lowpass(m, 6000);
  save('music.wav', s.master(m, 0.6));
}
console.log('tetherline assets written to', OUT);
