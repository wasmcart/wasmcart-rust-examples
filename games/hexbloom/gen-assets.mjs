#!/usr/bin/env node
// Hexbloom's sounds and music, generated from code (original, CC0).
// Re-run after editing; the WAVs are committed.
//
//   node games/hexbloom/gen-assets.mjs
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { writeWav } from '../../scripts/lib/wav.mjs';
import * as s from '../../scripts/lib/synth.mjs';

const OUT = path.join(path.dirname(fileURLToPath(import.meta.url)), 'assets');
const save = (name, b) => writeWav(path.join(OUT, name), s.RATE, b);
const R = s.RATE;

// shoot: a short soft pluck that falls in pitch.
{
  const b = s.buf(0.09);
  s.tone(b, 0, 0.05, 84, { wave: 'tri', vol: 0.5, slide: -12, env: { a: 0.002, d: 0.04, s: 0.2, r: 0.03 } });
  save('shoot.wav', s.fade(b, 1, 10));
}
// pop: an enemy bursts, noisy pitch drop.
{
  const b = s.buf(0.3);
  s.tone(b, 0, 0.18, 72, { wave: 'square', vol: 0.25, slide: -24, env: { a: 0.002, d: 0.1, s: 0.3, r: 0.08 } });
  for (let i = 0; i < b.length; i++) b[i] += 0.25 * s.noise() * Math.exp(-i / R * 22);
  s.lowpass(b, (i) => 6000 * Math.exp(-i / R * 8) + 300);
  save('pop.wav', s.fade(s.master(b, 0.7)));
}
// pickup: a rising sparkle.
{
  const b = s.buf(0.22);
  [88, 92, 95].forEach((n, k) => s.tone(b, k * 0.04, 0.06, n, { wave: 'sine', vol: 0.35, env: { a: 0.002, d: 0.05, s: 0.3, r: 0.06 } }));
  save('pickup.wav', s.fade(b));
}
// hurt: a low thump with grit.
{
  const b = s.buf(0.6);
  s.tone(b, 0, 0.35, 45, { wave: 'saw', vol: 0.5, slide: -10, env: { a: 0.003, d: 0.3, s: 0.2, r: 0.2 } });
  for (let i = 0; i < b.length; i++) b[i] += 0.3 * s.noise() * Math.exp(-i / R * 10);
  s.lowpass(b, 1200);
  save('hurt.wav', s.fade(s.master(b, 0.8)));
}
// bomb: a swelling whoosh into a low bloom.
{
  const b = s.buf(1.1);
  for (let i = 0; i < b.length; i++) {
    const t = i / R;
    b[i] = s.noise() * Math.min(1, t * 8) * Math.exp(-t * 3) * 0.7;
  }
  s.lowpass(b, (i) => 300 + 3000 * Math.exp(-i / R * 4));
  s.tone(b, 0, 0.6, 36, { wave: 'sine', vol: 0.6, slide: -7, env: { a: 0.01, d: 0.4, s: 0.4, r: 0.4 } });
  save('bomb.wav', s.fade(s.master(b, 0.8), 2, 60));
}
// wave: a bright three-note fanfare when a wave is cleared.
{
  const b = s.buf(1.0);
  [[72, 0], [76, 0.12], [79, 0.24], [84, 0.36]].forEach(([n, t]) =>
    s.tone(b, t, t === 0.36 ? 0.4 : 0.1, n, { wave: 'pulse25', vol: 0.18, detune: 8, env: { a: 0.005, d: 0.1, s: 0.6, r: 0.2 } }));
  s.echo(b, 0.18, 0.3, 0.3);
  save('wave.wav', s.fade(b));
}
// bosshit: a metallic tick for hits on a boss.
{
  const b = s.buf(0.08);
  s.tone(b, 0, 0.03, 96, { wave: 'square', vol: 0.15, slide: -5, env: { a: 0.001, d: 0.03, s: 0.1, r: 0.03 } });
  save('bosshit.wav', s.fade(b, 1, 10));
}

// music: a driving loop in D minor, 128 bpm, 8 bars.
function arena(bpm, root, dark) {
  return s.loop(bpm, 8, ({ out, beat }) => {
    const prog = dark ? [0, -2, -4, -5, 0, -2, 1, -5] : [0, 3, -2, 5, 0, 3, -4, -5];
    for (let bar = 0; bar < 8; bar++) {
      const r = root + prog[bar];
      for (let k = 0; k < 8; k++) {
        const t = beat(bar * 4 + k * 0.5);
        // bass: octave-bouncing eighths
        s.tone(out, t, 0.2, r - 12 + (k % 2 ? 12 : 0), { wave: 'saw', vol: 0.16, env: { a: 0.003, d: 0.1, s: 0.5, r: 0.05 } });
        s.hat(out, t + beat(0.25), 0.05);
      }
      for (let q = 0; q < 4; q++) {
        if (q % 2 === 0) s.kick(out, beat(bar * 4 + q), 0.55);
        else s.snare(out, beat(bar * 4 + q), 0.22);
      }
      // arpeggio lead over the chord, minor triad
      const arp = [0, 3, 7, 12, 7, 3, 0, 10];
      for (let k = 0; k < 8; k++) {
        const t = beat(bar * 4 + k * 0.5);
        s.tone(out, t, 0.18, r + 12 + arp[(k + bar) % 8], { wave: 'pulse25', vol: 0.05, env: { a: 0.004, d: 0.08, s: 0.4, r: 0.12 } });
      }
      // pad
      for (const iv of [0, 3, 7]) s.tone(out, beat(bar * 4), beat(3.8), r + iv, { wave: 'tri', vol: 0.05, detune: 10, env: { a: 0.3, d: 0.5, s: 0.7, r: 0.3 } });
    }
  });
}
{
  const m = arena(128, 50, false);
  s.lowpass(m, 7000);
  save('music.wav', s.master(m, 0.7));
}
{
  const m = arena(140, 47, true);
  s.lowpass(m, 6000);
  save('boss.wav', s.master(m, 0.72));
}
console.log('hexbloom assets written to', OUT);
